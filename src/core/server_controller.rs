//! Manages the host's local co-op server (`shadnet.exe --watchdog`).

use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::core::coop_schema;
use crate::core::health;
use crate::core::log;
use crate::core::server_assets;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
const FILE_SHARE_DELETE: u32 = 0x0000_0004;

struct Inner {
    child: Option<Child>,
    started_by_us: bool,
    log_path: PathBuf,
}

pub struct ServerController {
    inner: Mutex<Inner>,
}

impl Default for ServerController {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerController {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                child: None,
                started_by_us: false,
                log_path: server_assets::log_path(),
            }),
        }
    }

    pub fn started_by_us(&self) -> bool {
        self.inner.lock().map(|g| g.started_by_us).unwrap_or(false)
    }

    pub fn running(&self) -> bool {
        health::tcp("127.0.0.1", coop_schema::RPC_PORT, 400)
    }

    pub fn backend_answers(&self) -> bool {
        health::tcp("127.0.0.1", coop_schema::BACKEND_PORT, 400)
    }

    pub fn state(&self) -> &'static str {
        if !self.running() {
            return "parado";
        }
        if self.backend_answers() {
            "em execução"
        } else {
            "em execução, mas o backend (20443) não responde"
        }
    }

    pub fn start(&self, host_ip: &str) -> Result<String, String> {
        if self.running() {
            return Ok("O servidor já estava em execução.".to_string());
        }

        server_assets::ensure()?;

        let server_dir = server_assets::folder();
        let exe = server_assets::exe_path();
        if !exe.is_file() {
            return Err(format!(
                "shadnet.exe não está em {}.",
                server_dir.display()
            ));
        }

        let log_path = server_assets::log_path();
        if let Err(err) = fs::create_dir_all(server_assets::log_folder()) {
            log::line(&format!("Log do servidor não preparado ({err})."));
        }
        if log_path.is_file() {
            let _ = fs::copy(&log_path, server_assets::prev_log_path());
        }
        let _ = fs::create_dir_all(server_assets::backend_folder());

        // Open shadnet.txt with shared read/write so the UI can tail it while shadnet runs,
        // and pass the file handle directly as stdout & stderr (no cmd.exe wrapper needed!).
        let stdout_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(&log_path)
            .map_err(|e| format!("Não abri o log do servidor: {e}"))?;
        let stderr_file = stdout_file
            .try_clone()
            .map_err(|e| format!("Não dupliquei o handle do log: {e}"))?;

        let child = Command::new(&exe)
            .arg("--watchdog")
            .current_dir(&server_dir)
            .env(coop_schema::ENV_BACKEND_ADDRESS, host_ip)
            .env(
                coop_schema::ENV_RELEASE_VERSION,
                coop_schema::RELEASE_VERSION,
            )
            .env(coop_schema::ENV_QT_STDERR_LOGGING, "1")
            .env(coop_schema::ENV_QT_LOGGING_RULES, "*.debug=false")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file))
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn()
            .map_err(|e| format!("Não iniciei o servidor: {e}"))?;

        if let Ok(mut guard) = self.inner.lock() {
            guard.child = Some(child);
            guard.started_by_us = true;
            guard.log_path = log_path.clone();
        }
        log::line(&format!(
            "Servidor iniciado: {} --watchdog (backend em {}, log {}).",
            exe.display(),
            host_ip,
            log_path.display()
        ));
        Ok("Servidor iniciado (shadnet).".to_string())
    }

    pub fn wait_until_ready(&self, host_ip: &str, seconds: u64) -> String {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            if self.running() && self.backend_answers() {
                break;
            }
            thread::sleep(Duration::from_millis(300));
        }
        if !self.running() {
            return "O servidor não respondeu na porta 31313. Veja o log do servidor.".to_string();
        }
        if !self.backend_answers() {
            return "O servidor responde, mas o backend (20443) não. Pare e inicie o servidor de novo."
                .to_string();
        }
        format!("Servidor pronto · {}", health::describe(host_ip, 900))
    }

    pub fn stop(&self) -> String {
        let pid = {
            if let Ok(mut guard) = self.inner.lock() {
                guard.child.take().map(|c| c.id())
            } else {
                None
            }
        };

        if let Some(pid) = pid {
            run_taskkill(&["/PID", &pid.to_string(), "/T", "/F"]);
        }

        // The watchdog restarts a crashed child, so kill by image name too until 31313 stops.
        for _ in 0..8 {
            if !self.running() {
                break;
            }
            run_taskkill(&["/IM", "shadnet.exe", "/T", "/F"]);
            thread::sleep(Duration::from_millis(600));
        }

        if let Ok(mut guard) = self.inner.lock() {
            guard.started_by_us = false;
        }
        let stopped = !self.running();
        log::line(if stopped {
            "Servidor parado."
        } else {
            "O servidor continua respondendo na 31313."
        });
        if stopped {
            "Servidor parado.".to_string()
        } else {
            "O servidor ainda responde na porta 31313 (feche-o pelo Gerenciador de Tarefas)."
                .to_string()
        }
    }

    pub fn recent(&self, lines: usize) -> String {
        let path = self
            .inner
            .lock()
            .map(|g| g.log_path.clone())
            .unwrap_or_else(|_| server_assets::log_path());
        tail_shared(&path, lines)
    }
}

fn tail_shared(path: &PathBuf, lines: usize) -> String {
    let Ok(mut file) = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(path)
    else {
        return String::new();
    };
    let mut raw = Vec::new();
    if file.read_to_end(&mut raw).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&raw).replace("\r\n", "\n");
    let all: Vec<&str> = text.lines().collect();
    let from = all.len().saturating_sub(lines);
    let mut out = String::new();
    for l in &all[from..] {
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn run_taskkill(args: &[&str]) {
    if let Err(err) = Command::new("taskkill.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        log::line(&format!("taskkill {} falhou ({err}).", args.join(" ")));
    }
}
