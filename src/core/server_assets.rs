//! Embedded BBSC: Yharnam Together 1.0.0 Windows server (shadnet.exe + Qt6 DLLs), packed by
//! build.rs into OUT_DIR/server.zip and unpacked into %LOCALAPPDATA%\BBRadmin\server\ on first host.

use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Component, Path, PathBuf};

use zip::ZipArchive;

use crate::core::app_config;
use crate::core::coop_schema;
use crate::core::log;

const SERVER_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/server.zip"));
const LICENSE_NAME: &str = "LICENSE-BBSC.txt";

pub fn folder() -> PathBuf {
    app_config::folder().join(coop_schema::SERVER_FOLDER)
}

pub fn exe_path() -> PathBuf {
    folder().join(coop_schema::SERVER_EXE)
}

pub fn log_folder() -> PathBuf {
    app_config::folder().join(coop_schema::SERVER_LOG_FOLDER)
}

pub fn log_path() -> PathBuf {
    log_folder().join(coop_schema::SERVER_LOG)
}

pub fn prev_log_path() -> PathBuf {
    log_folder().join(coop_schema::SERVER_PREV_LOG)
}

pub fn backend_folder() -> PathBuf {
    app_config::folder().join(coop_schema::BACKEND_FOLDER)
}

pub fn stamp_path() -> PathBuf {
    app_config::folder().join(coop_schema::SERVER_STAMP)
}

pub fn installed() -> bool {
    if !exe_path().is_file() {
        return false;
    }
    let dir = folder();
    for rel in coop_schema::SERVER_FILES {
        if !dir.join(rel).is_file() {
            return false;
        }
    }
    stamp() == coop_schema::RELEASE_VERSION
}

pub fn state() -> &'static str {
    if !exe_path().is_file() {
        return "servidor do co-op ainda não instalado (ele entra sozinho ao hospedar)";
    }
    if stamp() != coop_schema::RELEASE_VERSION {
        return "servidor do co-op instalado em versão antiga (hospede de novo para atualizar)";
    }
    "servidor do co-op 1.0.0 pronto"
}

fn stamp() -> String {
    fs::read_to_string(stamp_path())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Unpacks the embedded server beside BBRadmin's own files when not yet installed/up-to-date.
/// Copies over existing files without deleting `db\` or `link_key.bin`.
pub fn ensure() -> Result<String, String> {
    if installed() {
        return Ok("Servidor do co-op já instalado.".to_string());
    }

    let cursor = Cursor::new(SERVER_ZIP);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| format!("Não abri o pacote embutido do servidor: {e}"))?;

    let app_dir = app_config::folder();
    let server_dir = folder();
    fs::create_dir_all(&server_dir)
        .map_err(|e| format!("Não criei a pasta do servidor: {e}"))?;

    let mut extracted_exe = false;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Erro lendo pacote do servidor: {e}"))?;
        let name = entry.name().replace('\\', "/");
        if entry.is_dir() {
            continue;
        }

        // Safely map "server/..." -> %LOCALAPPDATA%\BBRadmin\server\...
        // and "LICENSE-BBSC.txt" -> %LOCALAPPDATA%\BBRadmin\LICENSE-BBSC.txt
        let target = if let Some(rest) = name.strip_prefix("server/") {
            let rel = sanitize_relative(rest)
                .ok_or_else(|| format!("Caminho inválido no pacote: {name}"))?;
            if rel == Path::new(coop_schema::SERVER_EXE) {
                extracted_exe = true;
            }
            server_dir.join(rel)
        } else if name == LICENSE_NAME {
            app_dir.join(LICENSE_NAME)
        } else {
            continue;
        };

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Não criei pasta {}: {e}", parent.display()))?;
        }
        let mut out = File::create(&target)
            .map_err(|e| format!("Não gravei {}: {e}", target.display()))?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("Não extraí {}: {e}", target.display()))?;
    }

    if !extracted_exe || !exe_path().is_file() {
        return Err(format!(
            "O pacote do servidor não trazia o {}.",
            coop_schema::SERVER_EXE
        ));
    }

    let backend = backend_folder();
    fs::create_dir_all(&backend)
        .map_err(|e| format!("Não criei a pasta backend: {e}"))?;
    let notice = backend.join(coop_schema::BACKEND_NOTICE);
    if !notice.exists() {
        let _ = fs::write(&notice, b"");
    }

    fs::write(
        stamp_path(),
        format!("{}\r\n", coop_schema::RELEASE_VERSION),
    )
    .map_err(|e| format!("Não gravei a versão do servidor: {e}"))?;

    log::line(&format!(
        "Servidor do co-op 1.0.0 instalado em {}.",
        server_dir.display()
    ));
    Ok("Servidor do co-op 1.0.0 instalado.".to_string())
}

fn sanitize_relative(rel: &str) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if clean.as_os_str().is_empty() {
        None
    } else {
        Some(clean)
    }
}
