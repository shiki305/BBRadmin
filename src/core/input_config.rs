//! <bbport>\input.ini — switches `mouse_to_joystick` on/off with a one-time .bbradmin-bak backup.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::bbport::BbportInfo;
use crate::core::coop_schema;
use crate::core::log;

pub const KEYBOARD: &str = "keyboard";
pub const CONTROLLER: &str = "controller";

const MOUSE_KEY: &str = "mouse_to_joystick";
const MOUSE_ON: &str = "right";
const BACKUP_SUFFIX: &str = ".bbradmin-bak";

pub fn path_in(port: &BbportInfo) -> PathBuf {
    Path::new(&port.root).join(coop_schema::INPUT_FILE)
}

pub fn backup_path_in(port: &BbportInfo) -> PathBuf {
    PathBuf::from(format!("{}{}", path_in(port).display(), BACKUP_SUFFIX))
}

pub fn current(port: &BbportInfo) -> String {
    let path = path_in(port);
    let Ok(text) = fs::read_to_string(&path) else {
        return String::new();
    };
    if text.is_empty() {
        return String::new();
    }
    if mouse_camera_on(&text) {
        KEYBOARD.to_string()
    } else {
        CONTROLLER.to_string()
    }
}

pub fn describe(port: &BbportInfo) -> String {
    let mode = current(port);
    if mode.is_empty() {
        return format!(
            "sem {} (o jogo usa os controles padrão dele)",
            coop_schema::INPUT_FILE
        );
    }
    if mode == KEYBOARD {
        return "teclado e mouse (o mouse gira a câmera; F7 liga/desliga no jogo)".to_string();
    }
    "controle (o mouse não mexe a câmera; as teclas continuam funcionando)".to_string()
}

pub fn apply(port: &BbportInfo, mode: &str) -> Result<String, String> {
    let path = path_in(port);
    let on = mode == KEYBOARD;
    let wanted = if on { "Teclado e mouse" } else { "Controle" };
    let wanted_long = if on {
        "teclado e mouse (o mouse gira a câmera)"
    } else {
        "controle (o mouse não mexe a câmera)"
    };

    if !path.is_file() {
        return Ok(format!(
            "Não mexi nos controles: {} não está em {} (o jogo usa os controles padrão dele).",
            coop_schema::INPUT_FILE,
            port.root
        ));
    }

    let raw = fs::read(&path)
        .map_err(|e| format!("Não escrevi {}: {e}", coop_schema::INPUT_FILE))?;
    let bom = raw.len() >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF;
    let body_bytes = if bom { &raw[3..] } else { &raw[..] };
    let text = String::from_utf8_lossy(body_bytes).into_owned();

    if mouse_camera_on(&text) == on {
        return Ok(format!(
            "Controles já estavam em {wanted} (nada mudou)."
        ));
    }

    let mut lines: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
    let mut found = false;
    for line in &mut lines {
        let ending = if line.ends_with('\r') { "\r" } else { "" };
        let body = if !ending.is_empty() {
            &line[..line.len() - 1]
        } else {
            line.as_str()
        };
        if key_of(body) != MOUSE_KEY {
            continue;
        }
        let value = value_of(body);
        *line = if on {
            format!("{MOUSE_KEY} = {value}{ending}")
        } else {
            format!("# {MOUSE_KEY} = {value}{ending}")
        };
        found = true;
    }

    let new_text = if !found && on {
        let mut t = text;
        if !t.is_empty() && !t.ends_with('\n') {
            t.push('\n');
        }
        t.push_str("# BBRadmin: o mouse gira a câmera (F7 liga/desliga no jogo)\n");
        t.push_str(&format!("{MOUSE_KEY} = {MOUSE_ON}\n"));
        t
    } else {
        lines.join("\n")
    };

    backup_once(&path);
    let mut out = Vec::with_capacity(new_text.len() + if bom { 3 } else { 0 });
    if bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    out.extend_from_slice(new_text.as_bytes());
    fs::write(&path, &out)
        .map_err(|e| format!("Não escrevi {}: {e}", coop_schema::INPUT_FILE))?;
    Ok(format!(
        "Controles agora em {wanted_long} ({}).",
        coop_schema::INPUT_FILE
    ))
}

pub fn restore(port: &BbportInfo) -> Result<String, String> {
    let path = path_in(port);
    let backup = backup_path_in(port);
    if !backup.is_file() {
        return Ok("Nada a restaurar: o BBRadmin ainda não mudou os controles.".to_string());
    }
    fs::copy(&backup, &path)
        .map_err(|e| format!("Não restaurei os controles: {e}"))?;
    let _ = fs::remove_file(&backup);
    Ok("Controles restaurados como estavam antes do BBRadmin.".to_string())
}

fn mouse_camera_on(text: &str) -> bool {
    for raw in text.split('\n') {
        let body = raw.trim_end_matches('\r').trim_start();
        if body.starts_with('#') {
            continue;
        }
        if key_of(body) == MOUSE_KEY {
            return true;
        }
    }
    false
}

fn key_of(body: &str) -> &str {
    let mut t = body.trim_start();
    while let Some(rest) = t.strip_prefix('#') {
        t = rest.trim_start();
    }
    match t.split_once('=') {
        Some((k, _)) => k.trim(),
        None => t.trim(),
    }
}

fn value_of(body: &str) -> &str {
    match body.split_once('=') {
        Some((_, v)) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                MOUSE_ON
            } else {
                trimmed
            }
        }
        None => MOUSE_ON,
    }
}

fn backup_once(path: &Path) {
    let target = PathBuf::from(format!("{}{}", path.display(), BACKUP_SUFFIX));
    if target.exists() {
        return;
    }
    if let Err(err) = fs::copy(path, &target) {
        log::line(&format!(
            "Backup do {} não feito ({err}).",
            coop_schema::INPUT_FILE
        ));
    }
}
