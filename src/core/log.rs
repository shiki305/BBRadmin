//! BBRadmin's own log: %LOCALAPPDATA%\BBRadmin\bbradmin.log (kept under ~512 KB, then rotated once).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::Local;

use crate::core::app_config;

const MAX_BYTES: u64 = 512 * 1024;
static GATE: Mutex<()> = Mutex::new(());

pub fn file_path() -> PathBuf {
    app_config::folder().join("bbradmin.log")
}

pub fn line(message: &str) {
    let Ok(_guard) = GATE.lock() else {
        return;
    };
    let folder = app_config::folder();
    let _ = fs::create_dir_all(&folder);
    let path = file_path();
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_BYTES {
            let previous = folder.join("bbradmin.log.1");
            let _ = fs::remove_file(&previous);
            let _ = fs::rename(&path, &previous);
        }
    }
    let stamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = write!(file, "{stamp}  {message}\r\n");
    }
}

pub fn tail(lines: usize) -> String {
    let path = file_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return String::new();
    };
    let all: Vec<&str> = content.lines().collect();
    let from = all.len().saturating_sub(lines);
    let mut out = String::new();
    for l in &all[from..] {
        out.push_str(l);
        out.push('\n');
    }
    out
}
