//! Automatic self-updater via GitHub Releases (`anreren/BBRadmin`).
//!
//! On Windows, a running executable cannot be overwritten in-place, but it CAN be renamed
//! (`BBRadmin.exe` -> `BBRadmin.exe.old`) while running. We download the new binary to
//! `BBRadmin.exe.new`, validate its PE header (`MZ`) and byte size, swap the files atomically
//! (rolling back on error), launch the new executable, and clean up `.old` on next startup.

use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use crate::core::log;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_LATEST_API: &str = "https://api.github.com/repos/anreren/BBRadmin/releases/latest";
const ASSET_NAME: &str = "BBRadmin.exe";
const MIN_BINARY_BYTES: u64 = 1_000_000; // sanity guard against error pages

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
    pub size: u64,
    pub html_url: String,
}

/// Removes leftover `.old` / `.new` files from a previous self-update.
pub fn cleanup_old_binary() {
    let Ok(exe) = env::current_exe() else {
        return;
    };
    for suffix in [".old", ".new"] {
        let candidate = with_suffix(&exe, suffix);
        if candidate.is_file() {
            let _ = fs::remove_file(&candidate);
        }
    }
}

/// Checks GitHub Releases for a newer version than `CURRENT_VERSION`.
/// Returns `Ok(Some(info))` if an update is available, `Ok(None)` if up-to-date.
pub fn check_latest() -> Result<Option<UpdateInfo>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .user_agent(&format!("BBRadmin/{CURRENT_VERSION}"))
        .build();

    let resp = agent
        .get(RELEASES_LATEST_API)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Falha ao consultar GitHub Releases: {e}"))?;

    let body = resp
        .into_string()
        .map_err(|e| format!("Falha ao ler resposta do GitHub: {e}"))?;
    let json: Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON inválido do GitHub: {e}"))?;

    if json.get("draft").and_then(|v| v.as_bool()).unwrap_or(false)
        || json
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        return Ok(None);
    }

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !is_newer(tag, CURRENT_VERSION) {
        return Ok(None);
    }

    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/anreren/BBRadmin/releases/latest")
        .to_string();

    let assets = json
        .get("assets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Release sem assets disponíveis.".to_string())?;

    let asset = assets
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|n| n.as_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(ASSET_NAME))
        })
        .ok_or_else(|| format!("Release {tag} não contém {ASSET_NAME}."))?;

    let download_url = asset
        .get("browser_download_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if download_url.is_empty() {
        return Err(format!("URL de download ausente na release {tag}."));
    }
    let size = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

    Ok(Some(UpdateInfo {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        download_url,
        size,
        html_url,
    }))
}

/// Downloads `info.download_url`, validates the PE binary, and swaps it with the running exe.
/// Returns the path of the updated executable (same path as `current_exe`).
pub fn download_and_apply(info: &UpdateInfo) -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("Não localizei o executável atual: {e}"))?;
    let new_path = with_suffix(&exe, ".new");
    let old_path = with_suffix(&exe, ".old");

    // Verify we can write in the executable's directory before downloading.
    let mut out_file = File::create(&new_path).map_err(|e| {
        format!(
            "Sem permissão de escrita na pasta do BBRadmin ({}): {e}",
            exe.parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        )
    })?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(60))
        .user_agent(&format!("BBRadmin/{CURRENT_VERSION}"))
        .build();

    let download_result = (|| -> Result<u64, String> {
        let resp = agent
            .get(&info.download_url)
            .call()
            .map_err(|e| format!("Falha ao baixar v{}: {e}", info.version))?;
        let mut reader = resp.into_reader();
        let written = io::copy(&mut reader, &mut out_file)
            .map_err(|e| format!("Falha ao gravar atualização: {e}"))?;
        drop(out_file);
        Ok(written)
    })();

    let written = match download_result {
        Ok(n) => n,
        Err(err) => {
            let _ = fs::remove_file(&new_path);
            return Err(err);
        }
    };

    // Validate downloaded file size and Windows PE ("MZ") header.
    if written < MIN_BINARY_BYTES || (info.size > 0 && written != info.size) {
        let _ = fs::remove_file(&new_path);
        return Err(format!(
            "Download incompleto ({written} bytes; esperado {}).",
            info.size
        ));
    }
    let mut magic = [0u8; 2];
    let valid_pe = File::open(&new_path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && &magic == b"MZ";
    if !valid_pe {
        let _ = fs::remove_file(&new_path);
        return Err("O arquivo baixado não é um executável Windows válido.".to_string());
    }

    // Swap running exe -> .old, .new -> exe (rolling back if second rename fails).
    let _ = fs::remove_file(&old_path);
    if let Err(err) = fs::rename(&exe, &old_path) {
        let _ = fs::remove_file(&new_path);
        return Err(format!("Não consegui preparar a substituição do executável: {err}"));
    }
    if let Err(err) = fs::rename(&new_path, &exe) {
        let _ = fs::rename(&old_path, &exe);
        let _ = fs::remove_file(&new_path);
        return Err(format!("Não consegui instalar o novo executável: {err}"));
    }

    log::line(&format!(
        "Atualização aplicada: v{CURRENT_VERSION} -> v{} ({}).",
        info.version,
        exe.display()
    ));
    Ok(exe)
}

pub fn relaunch(exe: &Path) -> Result<(), String> {
    Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Não consegui reabrir o BBRadmin atualizado: {e}"))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(suffix);
    PathBuf::from(os)
}

pub fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let clean = raw
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()?;
    let mut parts = clean.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let patch = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison() {
        assert!(is_newer("v2.1.0", "2.0.0"));
        assert!(is_newer("v2.0.1", "2.0.0"));
        assert!(is_newer("3.0.0", "2.9.9"));
        assert!(!is_newer("v2.0.0", "2.0.0"));
        assert!(!is_newer("v1.9.9", "2.0.0"));
        assert!(!is_newer("invalid", "2.0.0"));
    }
}
