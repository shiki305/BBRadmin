//! `--dry-run`: writes out everything a real start would do (including display mode & env vars)
//! to %LOCALAPPDATA%\BBRadmin\dry-run.txt and prints it to the parent console without changing anything.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;
use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};

use crate::core::app_config::{self, AppConfig};
use crate::core::bbport;
use crate::core::coop_schema;
use crate::core::firewall;
use crate::core::game_environment;
use crate::core::input_config;
use crate::core::log;
use crate::core::overrides;
use crate::core::radmin;
use crate::core::server_assets;

pub fn file_path() -> PathBuf {
    app_config::folder().join("dry-run.txt")
}

pub fn run(config: &AppConfig, print_to_console: bool) -> i32 {
    let mut report = String::new();
    let now = Local::now().format("%Y-%m-%d %H:%M:%S");
    report.push_str(&format!("BBRadmin (Rust) · dry-run · {now}\n\n"));

    let mut root = config.bbport_dir.clone();
    if root.is_empty() || !Path::new(&root).is_dir() {
        let guess = bbport::guess_folder();
        let suffix = if !guess.is_empty() {
            format!(" (encontrei esta: {guess})")
        } else {
            String::new()
        };
        report.push_str(&format!("Pasta do bbport: não escolhida{suffix}\n"));
        if !guess.is_empty() {
            root = guess;
        }
    }
    let port = bbport::inspect(&root);
    report.push_str(&format!(
        "Pasta do bbport: {}\n",
        if port.root.is_empty() {
            "(nenhuma)"
        } else {
            &port.root
        }
    ));
    report.push_str(&format!(
        "Versão: {}\n",
        if port.version.is_empty() {
            "(não lida)"
        } else {
            &port.version
        }
    ));
    report.push_str(&format!(
        "Pronto para o co-op: {}\n",
        if port.ready() { "sim" } else { "NÃO" }
    ));
    for m in &port.missing {
        report.push_str(&format!("  FALTA: {m}\n"));
    }
    for n in &port.notes {
        report.push_str(&format!("  aviso: {n}\n"));
    }
    report.push('\n');

    let networks = radmin::detect();
    report.push_str("Endereços deste PC:\n");
    if networks.is_empty() {
        report.push_str("  nenhum (o Radmin VPN está ligado?)\n");
    }
    for choice in &networks {
        let tag = if radmin::is_vpn(&choice.kind) {
            "[VPN] "
        } else {
            "      "
        };
        report.push_str(&format!("  {tag}{}\n", choice.describe()));
    }
    let host_ip = radmin::pick(&networks)
        .map(|c| c.ip.clone())
        .unwrap_or_default();
    report.push_str(&format!(
        "Endereço que um start usaria: {}\n\n",
        if host_ip.is_empty() {
            "(nenhum)"
        } else {
            &host_ip
        }
    ));

    if !port.ready() || host_ip.is_empty() {
        report.push_str("Sem pasta do bbport ou sem endereço: um start não seria possível.\n");
        report.push_str("O arquivo do bbport e o servidor NÃO foram tocados.\n");
        return finish(&report, print_to_console);
    }

    report.push_str(&format!(
        "Conta: {}  (senha: {})\n",
        config.account,
        mask(&config.password)
    ));
    report.push_str(&format!("Regras do host: {}\n", config.rules.describe()));
    let play_with_note = if !config.play_with.is_empty() {
        format!("  ·  um start deixaria: {}", config.play_with)
    } else {
        "  ·  um start não mexeria".to_string()
    };
    report.push_str(&format!(
        "Controles ({}): {}{}\n\n",
        coop_schema::INPUT_FILE,
        input_config::describe(&port),
        play_with_note
    ));

    let plan = game_environment::build(
        &port,
        &host_ip,
        &config.account,
        &config.password,
        &config.rules,
        config.display_mode,
    );
    report.push_str(&plan.report(false));

    report.push_str("\n--- o que um HOSPEDAR faria ---\n");
    report.push_str(&format!(
        "  iniciar: {} --watchdog\n",
        server_assets::exe_path().display()
    ));
    report.push_str(&format!(
        "  pasta:   {}  ({})\n",
        server_assets::folder().display(),
        server_assets::state()
    ));
    report.push_str(&format!(
        "  log:     {}\n",
        server_assets::log_path().display()
    ));
    let missing_fw = firewall::missing();
    let fw_state = if missing_fw.is_empty() {
        "as três regras do BBRadmin estão instaladas".to_string()
    } else {
        format!("faltam: {}", missing_fw.join(", "))
    };
    report.push_str(&format!("  firewall: {fw_state}\n\n"));

    report.push_str("--- comandos de firewall (para um console de administrador) ---\n");
    report.push_str(&firewall::manual_commands());
    report.push('\n');

    let current = overrides::current(&port);
    report.push_str(&format!("--- {} agora ---\n", coop_schema::OVERRIDES_FILE));
    if current.trim().is_empty() {
        report.push_str("(não existe)\n");
    } else {
        report.push_str(current.trim_end());
        report.push('\n');
    }
    report.push_str("\nUm start real gravaria o arquivo acima (com backup ao lado) e iniciaria:\n");
    report.push_str(&format!(
        "  {} --run\n\n",
        Path::new(&port.root).join(coop_schema::GAME_EXE).display()
    ));
    report.push_str("Nada foi iniciado nem gravado por este dry-run.\n");

    finish(&report, print_to_console)
}

fn mask(password: &str) -> String {
    if password.is_empty() {
        "(vazia)".to_string()
    } else {
        "•".repeat(password.chars().count().min(12))
    }
}

fn finish(report: &str, print_to_console: bool) -> i32 {
    let dir = app_config::folder();
    let _ = fs::create_dir_all(&dir);
    let path = file_path();
    if let Err(err) = fs::write(&path, report.as_bytes()) {
        log::line(&format!("dry-run não gravado ({err})."));
    }
    if print_to_console {
        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                let mut stdout = std::io::stdout();
                let _ = write!(
                    stdout,
                    "{report}Relatório também em: {}\n",
                    path.display()
                );
                let _ = stdout.flush();
            }
        }
        if env::var("BBRADMIN_VERBOSE").as_deref() == Ok("1") {
            show_message_box("BBRadmin — dry-run", report);
        }
    }
    0
}

pub fn show_message_box(title: &str, text: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONINFORMATION, MB_OK,
    };
    let t: Vec<u16> = OsStr::new(title).encode_wide().chain(Some(0)).collect();
    let body: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body.as_ptr(),
            t.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
