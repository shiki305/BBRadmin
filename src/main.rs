#![windows_subsystem = "windows"]

mod core;
mod ui;

use std::env;
use std::fs;
use std::process::ExitCode;

use crate::core::app_config::{self, AppConfig};
use crate::core::dry_run;
use crate::core::firewall;
use crate::core::log;

fn main() -> ExitCode {
    let mut is_dry_run = false;
    let mut is_install_firewall = false;

    for raw in env::args().skip(1) {
        let name = raw
            .trim_start_matches(['-', '/'])
            .to_ascii_lowercase();
        if name == "dry-run" || name == "dryrun" {
            is_dry_run = true;
        } else if name == "install-firewall" {
            is_install_firewall = true;
        }
    }

    if is_install_firewall {
        return ExitCode::from(install_firewall() as u8);
    }

    let config = AppConfig::load();

    if is_dry_run {
        return ExitCode::from(dry_run::run(&config, true) as u8);
    }

    if let Err(err) = ui::run(config) {
        log::line(&format!("Erro fatal na janela: {err}"));
        dry_run::show_message_box("BBRadmin — erro", &format!("Não foi possível abrir a janela:\n{err}"));
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

fn install_firewall() -> i32 {
    let (ok, report) = firewall::install_all();
    let dir = app_config::folder();
    let _ = fs::create_dir_all(&dir);
    if let Err(err) = fs::write(app_config::firewall_report_path(), report.as_bytes()) {
        log::line(&format!("Relatório do firewall não gravado ({err})."));
    }
    if ok { 0 } else { 1 }
}
