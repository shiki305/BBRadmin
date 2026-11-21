//! Display mode resolution and Win32 WindowPlacer — direct Rust port of bbport's `bbport_display.py`.
//!
//! Why this exists (root cause of the fullscreen bug in original BBRadmin):
//! 1. `bb-probe-seamless.exe` (`gpu/shim/window.cpp`) does NOT read fullscreen settings from `bbport.ini`.
//!    It reads `BB_FULLSCREEN` and `BB_WINDOW_MODE` from the process environment (set by bbport's launcher
//!    from `%APPDATA%\bbport-launcher\settings.json`). Without them, it always opens in windowed mode.
//! 2. In bbport, "Borderless fullscreen" (`borderless_fullscreen: true`, `fullscreen: false` in
//!    `settings.json`) is implemented by the launcher itself (`bbport_display.Placer`), which finds the
//!    game's HWND, strips its title bar / borders (`WS_CAPTION | WS_THICKFRAME | WS_MAXIMIZEBOX` and
//!    extended edges), disables Windows 11 rounded corners (`DWMWA_WINDOW_CORNER_PREFERENCE`), and sizes
//!    it to cover the chosen monitor's full `rcMonitor` rect in physical pixels (`PER_MONITOR_AWARE_V2`).
//!    Original BBRadmin neither passed `BB_FULLSCREEN`/`BB_WINDOW_MODE` nor ran a window placer.
//!
//! This module fixes BOTH: it resolves the display mode (from bbport's `settings.json` or an explicit
//! override in BBRadmin) and runs a background Win32 `WindowPlacer` targeting BOTH `bb-probe-seamless.exe`
//! and `bb-probe.exe` while the game is running.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HDC, HMONITOR,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForMonitor, SetThreadDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, MDT_EFFECTIVE_DPI,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsIconic, IsWindowVisible, IsZoomed, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
    GWL_STYLE, MONITORINFOF_PRIMARY, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
    SWP_NOZORDER, SW_RESTORE, WS_CAPTION, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME,
    WS_EX_STATICEDGE, WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX, WS_THICKFRAME,
};

use crate::core::bbport;
use crate::core::bbport_launcher::LauncherSettings;
use crate::core::log;

const MIN_W: i32 = 640;
const MIN_H: i32 = 360;
const MAX_W: i32 = 7680;
const MAX_H: i32 = 4320;

const FRAME_MASK: isize = (WS_CAPTION | WS_THICKFRAME | WS_MAXIMIZEBOX) as isize;
const EDGES_MASK: isize =
    (WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE) as isize;

#[derive(Debug, Clone)]
pub struct ResolvedDisplay {
    pub bb_fullscreen: bool,
    pub exclusive: bool,
    pub borderless: bool,
    pub device: String,
    pub window_size: Option<(i32, i32)>,
}

impl ResolvedDisplay {
    pub fn describe(&self) -> String {
        let mode = if self.bb_fullscreen && self.exclusive {
            "Tela cheia exclusiva (BB_FULLSCREEN=1, BB_WINDOW_MODE=fullscreen)"
        } else if self.bb_fullscreen {
            "Tela cheia (BB_FULLSCREEN=1)"
        } else if self.borderless {
            "Tela cheia sem bordas (borderless via gerenciador de janela)"
        } else {
            "Janela (BB_FULLSCREEN=0)"
        };
        if self.device.is_empty() {
            mode.to_string()
        } else {
            format!("{mode} · monitor {}", self.device)
        }
    }
}

/// Resolves the effective display mode directly from bbport launcher's `settings.json`.
pub fn resolve(launcher: &LauncherSettings, output_res: (i32, i32)) -> ResolvedDisplay {
    let (bb_fullscreen, exclusive, borderless) = if launcher.fullscreen {
        (true, launcher.exclusive_fullscreen, false)
    } else if launcher.borderless_fullscreen {
        (false, false, true)
    } else {
        (false, false, false)
    };

    let window_size = window_target(launcher, output_res);

    ResolvedDisplay {
        bb_fullscreen,
        exclusive,
        borderless,
        device: launcher.game_monitor.clone(),
        window_size,
    }
}

pub fn parse_size(text: &str) -> Option<(i32, i32)> {
    let cleaned = text
        .trim()
        .replace('×', "x")
        .replace('*', "x")
        .replace('X', "x");
    let (w_s, h_s) = cleaned.split_once('x')?;
    let w = w_s.trim().parse::<i32>().ok()? / 2 * 2;
    let h = h_s.trim().parse::<i32>().ok()? / 2 * 2;
    if (MIN_W..=MAX_W).contains(&w) && (MIN_H..=MAX_H).contains(&h) {
        Some((w, h))
    } else {
        None
    }
}

fn window_target(launcher: &LauncherSettings, output: (i32, i32)) -> Option<(i32, i32)> {
    let mode = if launcher.window_size.is_empty() {
        "output"
    } else {
        launcher.window_size.as_str()
    };
    if mode == "game" {
        return None;
    }
    let custom = match mode {
        "custom" => parse_size(&launcher.window_custom),
        "last" => parse_size(&launcher.window_last),
        _ => None,
    };
    Some(custom.unwrap_or(output))
}

#[derive(Debug, Clone)]
struct Monitor {
    device: String,
    rect: (i32, i32, i32, i32),
    work: (i32, i32, i32, i32),
    primary: bool,
}

impl Monitor {
    fn size(&self) -> (i32, i32) {
        (self.rect.2 - self.rect.0, self.rect.3 - self.rect.1)
    }

    fn number(&self) -> u32 {
        let digits: String = self.device.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse().unwrap_or(0)
    }
}

/// Handle to a running background Win32 WindowPlacer thread. Dropping or calling `stop()` ends it.
pub struct WindowPlacer {
    stop: Arc<AtomicBool>,
}

impl WindowPlacer {
    pub fn start(cfg: ResolvedDisplay) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);
        thread::spawn(move || {
            run_placer(cfg, stop_clone);
        });
        Self { stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for WindowPlacer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_placer(cfg: ResolvedDisplay, stop: Arc<AtomicBool>) {
    // Measure and move windows in physical pixels (Per-Monitor DPI Aware v2), matching bbport_display.py.
    let old_dpi = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let mut placed: HashSet<isize> = HashSet::new();
    let mut seen: HashMap<isize, (i32, i32, i32, i32)> = HashMap::new();

    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(250));
        if stop.load(Ordering::Relaxed) {
            break;
        }
        for hwnd in find_game_windows() {
            let key = hwnd as isize;
            let rect = get_window_rect(hwnd);
            if !placed.contains(&key) {
                // Wait until the window rect stops moving between two 250ms ticks so SDL finishes creating it.
                if seen.get(&key) == Some(&rect) && (rect.2 - rect.0) > 100 && (rect.3 - rect.1) > 100
                {
                    place_window(hwnd, &cfg);
                    placed.insert(key);
                }
                seen.insert(key, rect);
            } else if cfg.borderless {
                keep_borderless(hwnd);
            }
        }
    }

    if !old_dpi.is_null() {
        unsafe {
            SetThreadDpiAwarenessContext(old_dpi);
        }
    }
}

fn find_game_windows() -> Vec<HWND> {
    let pids = bbport::find_pids_by_names(&["bb-probe-seamless.exe", "bb-probe.exe"]);
    if pids.is_empty() {
        return Vec::new();
    }
    struct Ctx {
        pids: Vec<u32>,
        found: Vec<HWND>,
    }
    let mut ctx = Ctx {
        pids,
        found: Vec::new(),
    };
    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut Ctx);
        let mut owner = 0u32;
        GetWindowThreadProcessId(hwnd, &mut owner);
        if ctx.pids.contains(&owner) && IsWindowVisible(hwnd) != 0 {
            ctx.found.push(hwnd);
        }
        1
    }
    unsafe {
        EnumWindows(Some(enum_cb), &mut ctx as *mut Ctx as LPARAM);
    }
    ctx.found
}

fn enum_monitors() -> Vec<Monitor> {
    let mut found: Vec<Monitor> = Vec::new();
    unsafe extern "system" fn mon_cb(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let list = &mut *(lparam as *mut Vec<Monitor>);
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut _ as *mut _) != 0 {
            let r = info.monitorInfo.rcMonitor;
            let w = info.monitorInfo.rcWork;
            let len = info
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.szDevice.len());
            let device = String::from_utf16_lossy(&info.szDevice[..len]);
            list.push(Monitor {
                device,
                rect: (r.left, r.top, r.right, r.bottom),
                work: (w.left, w.top, w.right, w.bottom),
                primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
            });
        }
        1
    }
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(mon_cb),
            &mut found as *mut Vec<Monitor> as LPARAM,
        );
    }
    found.sort_by_key(|m| (!m.primary, m.number()));
    found
}

fn pick_monitor(device: &str) -> Option<Monitor> {
    let all = enum_monitors();
    if !device.is_empty() {
        if let Some(m) = all.iter().find(|m| m.device == device) {
            return Some(m.clone());
        }
    }
    all.iter()
        .find(|m| m.primary)
        .or_else(|| all.first())
        .cloned()
}

fn get_window_rect(hwnd: HWND) -> (i32, i32, i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetWindowRect(hwnd, &mut r);
    }
    (r.left, r.top, r.right, r.bottom)
}

fn get_client_size(hwnd: HWND) -> (i32, i32) {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut r);
    }
    (r.right, r.bottom)
}

fn monitor_rect_of(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let hmon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        return None;
    }
    let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(hmon, &mut info.monitorInfo as *mut _ as *mut _) } == 0 {
        return None;
    }
    let r = info.monitorInfo.rcMonitor;
    Some((r.left, r.top, r.right, r.bottom))
}

fn make_borderless(hwnd: HWND, rect: (i32, i32, i32, i32)) -> bool {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let new_style = style & !FRAME_MASK;
    let new_ex_style = ex_style & !EDGES_MASK;
    let framed = new_style != style || new_ex_style != ex_style;
    if framed {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
        }
    }
    if !framed && get_window_rect(hwnd) == rect {
        return false;
    }
    let flags = SWP_NOZORDER
        | SWP_NOACTIVATE
        | SWP_NOOWNERZORDER
        | if framed { SWP_FRAMECHANGED } else { 0 };
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.0,
            rect.1,
            rect.2 - rect.0,
            rect.3 - rect.1,
            flags,
        );
    }
    true
}

fn square_corners(hwnd: HWND) {
    let corner: i32 = DWMWCP_DONOTROUND;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            &corner as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );
    }
}

fn keep_borderless(hwnd: HWND) {
    if unsafe { IsIconic(hwnd) } != 0 {
        return;
    }
    if unsafe { IsZoomed(hwnd) } != 0 {
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
        }
    }
    if let Some(rect) = monitor_rect_of(hwnd) {
        make_borderless(hwnd, rect);
    }
}

fn monitor_dpi(m: &Monitor) -> u32 {
    let pt = POINT {
        x: m.rect.0 + 1,
        y: m.rect.1 + 1,
    };
    let hmon = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        return 96;
    }
    let (mut x, mut y) = (0u32, 0u32);
    if unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut x, &mut y) } == 0 && x > 0 {
        x
    } else {
        96
    }
}

fn frame_for(hwnd: HWND, client: (i32, i32), dpi: u32) -> (i32, i32) {
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: client.0,
        bottom: client.1,
    };
    unsafe {
        AdjustWindowRectExForDpi(&mut rect, style, 0, ex_style, dpi);
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

fn fit(size: (i32, i32), room: (i32, i32)) -> (i32, i32) {
    let (w, h) = (size.0.max(2) as f64, size.1.max(2) as f64);
    let scale = (room.0.max(2) as f64 / w)
        .min(room.1.max(2) as f64 / h)
        .min(1.0);
    let nw = ((w * scale) as i32 / 2 * 2).max(2);
    let nh = ((h * scale) as i32 / 2 * 2).max(2);
    (nw, nh)
}

fn place_window(hwnd: HWND, cfg: &ResolvedDisplay) {
    let Some(m) = pick_monitor(&cfg.device) else {
        return;
    };

    if cfg.borderless {
        if unsafe { IsZoomed(hwnd) } != 0 {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
            }
        }
        make_borderless(hwnd, m.rect);
        square_corners(hwnd);
        log::line(&format!(
            "Janela do jogo ajustada para tela cheia sem bordas em {} ({}x{}).",
            m.device,
            m.size().0,
            m.size().1
        ));
        return;
    }

    if cfg.bb_fullscreen {
        if get_window_rect(hwnd) != m.rect {
            let (w, h) = m.size();
            unsafe {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    m.rect.0,
                    m.rect.1,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            log::line(&format!(
                "Janela em tela cheia movida para o monitor {} ({}x{}).",
                m.device, w, h
            ));
        }
        return;
    }

    // Windowed mode: respect monitor and chosen window size (if any).
    if unsafe { IsZoomed(hwnd) } != 0 {
        return;
    }
    if cfg.window_size.is_none() && (cfg.device.is_empty() || m.primary) {
        return;
    }
    let dpi = monitor_dpi(&m);
    let work = (m.work.2 - m.work.0, m.work.3 - m.work.1);
    let base_frame = frame_for(hwnd, (100, 100), dpi);
    let extra = (base_frame.0 - 100, base_frame.1 - 100);
    let desired = cfg.window_size.unwrap_or_else(|| get_client_size(hwnd));
    let client = fit(desired, (work.0 - extra.0, work.1 - extra.1));
    for _ in 0..2 {
        let outer = frame_for(hwnd, client, dpi);
        let x = m.work.0 + ((work.0 - outer.0) / 2).max(0);
        let y = m.work.1 + ((work.1 - outer.1) / 2).max(0);
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                x,
                y,
                outer.0,
                outer.1,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        thread::sleep(Duration::from_millis(200));
        if get_client_size(hwnd) == client {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_launcher_borderless_and_fullscreen_modes() {
        let mut launcher = LauncherSettings::default();
        launcher.exists = true;

        // fullscreen=false, borderless_fullscreen=true
        launcher.fullscreen = false;
        launcher.borderless_fullscreen = true;
        let r = resolve(&launcher, (1920, 1080));
        assert!(!r.bb_fullscreen);
        assert!(!r.exclusive);
        assert!(r.borderless);

        // fullscreen=true, exclusive=false
        launcher.fullscreen = true;
        launcher.exclusive_fullscreen = false;
        let r = resolve(&launcher, (1920, 1080));
        assert!(r.bb_fullscreen);
        assert!(!r.exclusive);
        assert!(!r.borderless);

        // fullscreen=true, exclusive=true
        launcher.exclusive_fullscreen = true;
        let r = resolve(&launcher, (1920, 1080));
        assert!(r.bb_fullscreen);
        assert!(r.exclusive);
        assert!(!r.borderless);

        // windowed
        launcher.fullscreen = false;
        launcher.borderless_fullscreen = false;
        let r = resolve(&launcher, (1920, 1080));
        assert!(!r.bb_fullscreen);
        assert!(!r.borderless);
    }

    #[test]
    fn parses_resolution_sizes() {
        assert_eq!(parse_size("1920x1080"), Some((1920, 1080)));
        assert_eq!(parse_size("2560 × 1440"), Some((2560, 1440)));
        assert_eq!(parse_size("100x100"), None);
    }
}
