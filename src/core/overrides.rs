//! <bbport>\seamless\user\host_overrides.custom.json — redirects Bloodborne's game server
//! (https://ss4.scej-network.jp:20443) to the host's co-op backend (http://<hostIp>:20443).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;

use crate::core::bbport::BbportInfo;
use crate::core::coop_schema;
use crate::core::log;

const BACKUP_SUFFIX: &str = ".bbradmin-bak";
const STAMP_PREFIX: &str = "bbradmin-";

pub fn path_in(port: &BbportInfo) -> PathBuf {
    Path::new(&port.root).join(coop_schema::OVERRIDES_FILE)
}

pub fn text(host_ip: &str) -> String {
    format!(
        "{{\n  \"{}\": \"http://{}:{}\"\n}}\n",
        coop_schema::GAME_SERVER,
        host_ip,
        coop_schema::BACKEND_PORT
    )
}

pub fn write(port: &BbportInfo, host_ip: &str) -> Result<String, String> {
    let path = path_in(port);
    let wanted = text(host_ip);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Não escrevi {}: {e}", coop_schema::OVERRIDES_FILE))?;
    }
    let current = fs::read_to_string(&path).unwrap_or_default();
    if normalize(&current) == normalize(&wanted) {
        return Ok(format!(
            "Overrides já apontavam para {host_ip} (nada mudou)."
        ));
    }
    if path.is_file() {
        backup(&path);
    }
    fs::write(&path, wanted.as_bytes())
        .map_err(|e| format!("Não escrevi {}: {e}", coop_schema::OVERRIDES_FILE))?;
    Ok(format!(
        "Overrides gravados: {} → http://{}:{}",
        coop_schema::OVERRIDES_FILE,
        host_ip,
        coop_schema::BACKEND_PORT
    ))
}

pub fn current(port: &BbportInfo) -> String {
    fs::read_to_string(path_in(port)).unwrap_or_default()
}

pub fn restore(port: &BbportInfo) -> Result<String, String> {
    let path = path_in(port);
    let Some(dir) = path.parent() else {
        return Ok("Nada a restaurar: a pasta do co-op não existe.".to_string());
    };
    if !dir.is_dir() {
        return Ok("Nada a restaurar: a pasta do co-op não existe.".to_string());
    }
    let base_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("host_overrides.custom.json");
    let prefix = format!("{base_name}{BACKUP_SUFFIX}");

    let mut backups: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("Não restaurei os overrides: {e}"))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();

    if backups.is_empty() {
        if path.is_file() {
            fs::remove_file(&path)
                .map_err(|e| format!("Não restaurei os overrides: {e}"))?;
            return Ok(
                "Overrides removidos (não havia backup: o bbport volta ao arquivo próprio dele)."
                    .to_string(),
            );
        }
        return Ok("Nada a restaurar.".to_string());
    }

    backups.sort();
    let newest = backups.last().unwrap();
    fs::copy(newest, &path).map_err(|e| format!("Não restaurei os overrides: {e}"))?;
    let _ = fs::remove_file(newest);
    let fname = newest
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("backup");
    Ok(format!("Overrides restaurados do backup {fname}."))
}

fn backup(path: &Path) {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let target = PathBuf::from(format!(
        "{}{}-{}{}",
        path.display(),
        BACKUP_SUFFIX,
        STAMP_PREFIX,
        stamp
    ));
    if let Err(err) = fs::copy(path, &target) {
        log::line(&format!("Backup dos overrides não feito ({err})."));
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}
