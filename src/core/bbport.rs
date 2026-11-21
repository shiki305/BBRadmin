//! Finds, inspects, and manages the local bbport installation (Supermedo/bloodborne_pc).

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, WAIT_OBJECT_0};
use windows_sys::Win32::Storage::FileSystem::{
    GetDriveTypeW, GetFileVersionInfoSizeW, GetFileVersionInfoW, GetLogicalDrives, VerQueryValueW,
    VS_FIXEDFILEINFO,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE,
};

use crate::core::bbport_launcher;
use crate::core::coop_schema;
use crate::core::log;

const DRIVE_FIXED: u32 = 3;

#[derive(Debug, Clone, Default)]
pub struct BbportInfo {
    pub root: String,
    pub version: String,
    pub coop_release: String,
    pub missing: Vec<String>,
    pub notes: Vec<String>,
}

impl BbportInfo {
    pub fn ready(&self) -> bool {
        !self.root.is_empty() && self.missing.is_empty()
    }

    pub fn describe_for(&self, lang: crate::core::i18n::Lang) -> String {
        use crate::core::i18n::t;
        if self.root.is_empty() {
            return t(
                lang,
                "pasta do bbport ainda não escolhida",
                "bbport folder not selected yet",
            )
            .to_string();
        }
        if !self.ready() {
            return format!(
                "{}: {}",
                t(lang, "faltando", "missing"),
                self.missing.join(", ")
            );
        }
        let ver = if self.version.is_empty() {
            "bbport".to_string()
        } else {
            format!("bbport {}", self.version)
        };
        let coop = if self.coop_release.is_empty() {
            String::new()
        } else {
            format!(" · co-op {}", self.coop_release)
        };
        format!("{ver}{coop} · {}", self.root)
    }
}

pub fn inspect(root: &str) -> BbportInfo {
    let mut info = BbportInfo::default();
    if root.is_empty() {
        return info;
    }
    info.root = root.to_string();
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        info.missing.push("a pasta não existe".to_string());
        return info;
    }

    require(&mut info, coop_schema::GAME_EXE, "Bloodborne.exe");
    require(
        &mut info,
        coop_schema::ENGINE_SEAMLESS,
        r"bin\bb-probe-seamless.exe",
    );
    require(&mut info, coop_schema::COOP_USER_DIR, r"seamless\user");

    note(
        &mut info,
        coop_schema::COOP_GAME_VIEW,
        r"seamless\coop-game\CUSA03173 (a view of the game: abra o Seamless co-op uma vez no launcher do bbport)",
    );
    note(
        &mut info,
        coop_schema::COOP_MODS_VIEW,
        r"seamless\mods-view (o co-op vem dela: abra o Seamless co-op uma vez no launcher do bbport)",
    );

    info.version = read_version(root_path);
    info.coop_release = read_coop_release(root_path);
    if !info.coop_release.is_empty() && info.coop_release != coop_schema::RELEASE_VERSION {
        info.notes.push(format!(
            "o co-op do bbport é a versão {}, e o servidor do BBRadmin é a {}: o servidor recusa um jogo de outra versão, então hospedar só funciona com as duas iguais",
            info.coop_release,
            coop_schema::RELEASE_VERSION
        ));
    }
    info
}

fn require(info: &mut BbportInfo, relative: &str, shown: &str) {
    let path = Path::new(&info.root).join(relative);
    if !path.exists() {
        info.missing.push(shown.to_string());
    }
}

fn note(info: &mut BbportInfo, relative: &str, shown: &str) {
    let path = Path::new(&info.root).join(relative);
    if !path.is_dir() {
        info.notes.push(shown.to_string());
    }
}

/// Reads `output_res` from `<bbport>\bbport.ini` (for the window placer), defaulting to (1920, 1080).
pub fn read_output_res(port: &BbportInfo) -> (i32, i32) {
    let path = Path::new(&port.root).join(coop_schema::BBPORT_INI);
    if let Ok(text) = fs::read_to_string(&path) {
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("output_res=") {
                if let Some(size) = crate::core::display::parse_size(val) {
                    return size;
                }
            }
        }
    }
    (1920, 1080)
}

fn read_version(root: &Path) -> String {
    for rel in [coop_schema::GAME_EXE, coop_schema::ENGINE_SEAMLESS] {
        let path = root.join(rel);
        if let Some(ver) = read_pe_version(&path) {
            return ver;
        }
    }
    let manifest = root.join("bbport-files.json");
    if let Ok(text) = fs::read_to_string(&manifest) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(ver) = val.get("version").and_then(|v| v.as_str()) {
                return ver.trim().to_string();
            }
        }
    }
    String::new()
}

fn read_pe_version(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut handle = 0u32;
    let size = unsafe { GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(wide.as_ptr(), 0, size, buf.as_mut_ptr() as _) } == 0 {
        return None;
    }
    let root_sub: Vec<u16> = OsStr::new("\\")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut ptr_val: *mut std::ffi::c_void = ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe {
        VerQueryValueW(
            buf.as_ptr() as _,
            root_sub.as_ptr(),
            &mut ptr_val,
            &mut len,
        )
    };
    if ok == 0 || ptr_val.is_null() || (len as usize) < std::mem::size_of::<VS_FIXEDFILEINFO>() {
        return None;
    }
    let info = unsafe { &*(ptr_val as *const VS_FIXEDFILEINFO) };
    let major = (info.dwFileVersionMS >> 16) & 0xFFFF;
    let minor = info.dwFileVersionMS & 0xFFFF;
    let build = (info.dwFileVersionLS >> 16) & 0xFFFF;
    let rev = info.dwFileVersionLS & 0xFFFF;
    if major == 0 && minor == 0 && build == 0 && rev == 0 {
        return None;
    }
    if rev != 0 {
        Some(format!("{major}.{minor}.{build}.{rev}"))
    } else if build != 0 {
        Some(format!("{major}.{minor}.{build}"))
    } else {
        Some(format!("{major}.{minor}"))
    }
}

fn read_coop_release(root: &Path) -> String {
    let path = root.join(coop_schema::COOP_RELEASE_FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return String::new();
    };
    // Look for vX.Y or vX.Y.Z
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.');
        if let Some(rest) = cleaned.strip_prefix('v') {
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) && rest.contains('.') {
                return rest.to_string();
            }
        }
    }
    String::new()
}

pub fn guess_folder() -> String {
    if let Some(found) = guess_from_settings() {
        return found;
    }
    for candidate in candidates() {
        if has_port(&candidate) {
            return candidate.to_string_lossy().to_string();
        }
    }
    const SKIP: &[&str] = &[
        "windows",
        "$recycle.bin",
        "program files",
        "program files (x86)",
        "programdata",
        "system volume information",
        "recovery",
        "perflogs",
        "msocache",
        "appdata",
    ];
    let mut budget = 6000i32;
    for drive in fixed_drives() {
        if let Some(found) = find_port(&drive, 0, 2, SKIP, &mut budget) {
            return found.to_string_lossy().to_string();
        }
        if budget <= 0 {
            break;
        }
    }
    String::new()
}

fn guess_from_settings() -> Option<String> {
    let settings = bbport_launcher::load();
    if settings.game_dir.is_empty() {
        return None;
    }
    let game_dir = Path::new(&settings.game_dir);
    let parent = game_dir.parent()?;
    if has_port(parent) {
        return Some(parent.to_string_lossy().to_string());
    }
    if let Some(grand) = parent.parent() {
        if has_port(grand) {
            return Some(grand.to_string_lossy().to_string());
        }
    }
    None
}

fn fixed_drives() -> Vec<PathBuf> {
    let mut drives = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if (mask & (1 << i)) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root_str = format!("{letter}:\\");
        let wide: Vec<u16> = OsStr::new(&root_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        if unsafe { GetDriveTypeW(wide.as_ptr()) } == DRIVE_FIXED {
            drives.push(PathBuf::from(root_str));
        }
    }
    drives
}

fn candidates() -> Vec<PathBuf> {
    let mut places = Vec::new();
    const LAYOUTS: &[&str] = &[
        r"Games\Bloodborne-Windows\bbport-windows",
        r"Games\Bloodborne-Windows",
        r"Games\bbport-windows",
        r"Bloodborne-Windows\bbport-windows",
        "bbport-windows",
        r"Games\bbport",
    ];
    for drive in fixed_drives() {
        for layout in LAYOUTS {
            places.push(drive.join(layout));
        }
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        let base = PathBuf::from(profile);
        for layout in [
            "bbport-windows",
            r"Downloads\bbport-windows",
            r"Documents\bbport-windows",
        ] {
            places.push(base.join(layout));
        }
    }
    places
}

fn find_port(
    folder: &Path,
    depth: usize,
    max_depth: usize,
    skip: &[&str],
    budget: &mut i32,
) -> Option<PathBuf> {
    if *budget <= 0 {
        return None;
    }
    *budget -= 1;
    if has_port(folder) {
        return Some(folder.to_path_buf());
    }
    if depth >= max_depth {
        return None;
    }
    let Ok(entries) = fs::read_dir(folder) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if skip.iter().any(|s| *s == name) {
            continue;
        }
        if let Some(found) = find_port(&path, depth + 1, max_depth, skip, budget) {
            return Some(found);
        }
        if *budget <= 0 {
            return None;
        }
    }
    None
}

fn has_port(folder: &Path) -> bool {
    folder.join(coop_schema::GAME_EXE).is_file()
        && folder.join(coop_schema::ENGINE_NORMAL).is_file()
}

/// Returns PIDs of running Bloodborne / bb-probe / bb-probe-seamless processes.
pub fn running_game_pids() -> Vec<u32> {
    find_pids_by_names(&["bloodborne.exe", "bb-probe.exe", "bb-probe-seamless.exe"])
}

pub fn find_pids_by_names(targets: &[&str]) -> Vec<u32> {
    let mut pids = Vec::new();
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap.is_null() || snap == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return pids;
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snap, &mut entry) };
    while ok != 0 {
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase();
        if targets.iter().any(|t| *t == name) {
            pids.push(entry.th32ProcessID);
        }
        ok = unsafe { Process32NextW(snap, &mut entry) };
    }
    unsafe {
        CloseHandle(snap);
    }
    pids
}

/// Closes running game processes politely (WM_CLOSE) first, then terminates if needed.
pub fn close_game(pids: &[u32]) -> usize {
    let mut closed = 0;
    for &pid in pids {
        // Send WM_CLOSE to top-level windows belonging to this PID.
        unsafe extern "system" fn close_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let target_pid = lparam as u32;
            let mut owner = 0u32;
            GetWindowThreadProcessId(hwnd, &mut owner);
            if owner == target_pid {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            1
        }
        unsafe {
            EnumWindows(Some(close_cb), pid as LPARAM);
        }

        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_TERMINATE, 0, pid) };
        if handle.is_null() {
            continue;
        }
        let waited = unsafe { WaitForSingleObject(handle, 8000) };
        if waited == WAIT_OBJECT_0 {
            closed += 1;
        } else {
            let term = unsafe { TerminateProcess(handle, 1) };
            if term != 0 {
                if unsafe { WaitForSingleObject(handle, 4000) } == WAIT_OBJECT_0 {
                    closed += 1;
                }
            } else {
                log::line(&format!("Não fechei o processo do jogo (pid {pid})."));
            }
        }
        unsafe {
            CloseHandle(handle);
        }
    }
    if closed > 0 {
        thread::sleep(Duration::from_millis(200));
    }
    closed
}
