//! BBRadmin's own settings, in %LOCALAPPDATA%\BBRadmin\config.ini.
//! Fully compatible with the original C# config.ini (including DPAPI-protected passwords),
//! plus an optional `display_mode` preference (`""` = follow bbport launcher).

use std::env;
use std::fs;
use std::path::PathBuf;

use rand::Rng;

use crate::core::bbport_launcher;
use crate::core::coop_schema;
use crate::core::dpapi;
use crate::core::host_rules::HostRules;
use crate::core::i18n::{t, Lang, LangMode};
use crate::core::input_config;
use crate::core::log;

pub const ROLE_HOST: &str = "host";
pub const ROLE_GUEST: &str = "guest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Dark,
    Light,
}

impl ThemeMode {
    pub fn from_config(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::System,
        }
    }

    pub fn as_config(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn short_label_for(self, lang: Lang) -> &'static str {
        match self {
            Self::System => t(lang, "Tema: Sistema", "Theme: System"),
            Self::Dark => t(lang, "Tema: Escuro", "Theme: Dark"),
            Self::Light => t(lang, "Tema: Claro", "Theme: Light"),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Dark,
            Self::Dark => Self::Light,
            Self::Light => Self::System,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bbport_dir: String,
    pub role: String,
    pub account: String,
    pub password: String,
    pub remote_ip: String,
    pub rules: HostRules,
    pub play_with: String,
    pub theme: ThemeMode,
    pub lang: LangMode,
    pub auto_update: bool,
    pub auto_stop_server: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            bbport_dir: String::new(),
            role: ROLE_HOST.to_string(),
            account: String::new(),
            password: String::new(),
            remote_ip: String::new(),
            rules: HostRules::default(),
            play_with: String::new(),
            theme: ThemeMode::System,
            lang: LangMode::Auto,
            auto_update: true,
            auto_stop_server: true,
        }
    }
}

pub fn folder() -> PathBuf {
    let local = env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local).join("BBRadmin")
}

pub fn file_path() -> PathBuf {
    folder().join("config.ini")
}

pub fn firewall_report_path() -> PathBuf {
    folder().join("firewall-report.txt")
}

impl AppConfig {
    pub fn load() -> Self {
        let mut config = Self::default();
        let launcher = bbport_launcher::load();
        config.rules = launcher.rules.clone();
        config.play_with = launcher.play_with.clone();

        let path = file_path();
        if let Ok(content) = fs::read_to_string(&path) {
            for raw in content.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                let key = k.trim();
                let value = v.trim();
                match key {
                    "bbport_dir" => config.bbport_dir = value.to_string(),
                    "role" => {
                        config.role = if value == ROLE_GUEST {
                            ROLE_GUEST.to_string()
                        } else {
                            ROLE_HOST.to_string()
                        };
                    }
                    "account" => config.account = value.to_string(),
                    "password" => config.password = dpapi::unprotect(value),
                    "remote_ip" => config.remote_ip = value.to_string(),
                    "rule_rot" => config.rules.rot = value == "1",
                    "rule_travel" => config.rules.travel = value == "1",
                    "rule_vote" => config.rules.vote = value == "1",
                    "rule_revive" => config.rules.revive = value != "0",
                    "rule_enemies" => config.rules.enemies = value.to_string(),
                    "rule_tempo" => config.rules.tempo = value.to_string(),
                    "rule_custom_hp" => config.rules.custom_hp = value.to_string(),
                    "rule_custom_damage" => config.rules.custom_damage = value.to_string(),
                    "rule_custom_poise" => config.rules.custom_poise = value.to_string(),
                    "play_with" => {
                        config.play_with =
                            if value == input_config::KEYBOARD || value == input_config::CONTROLLER
                            {
                                value.to_string()
                            } else {
                                String::new()
                            };
                    }
                    "theme" => config.theme = ThemeMode::from_config(value),
                    "ui_lang" => config.lang = LangMode::from_config(value),
                    "auto_update" => config.auto_update = value != "0",
                    "auto_stop_server" => config.auto_stop_server = value != "0",
                    // Backwards compatibility with older config.ini files:
                    "keep_server" => {
                        if value == "1" {
                            config.auto_stop_server = false;
                        }
                    }
                    "stop_server_with_game" => {
                        if value == "0" {
                            config.auto_stop_server = false;
                        }
                    }
                    _ => {}
                }
            }
        } else if path.exists() {
            log::line("Configuração não lida; usando os valores padrão.");
        }

        if !coop_schema::valid_account(&config.account) {
            config.account = if coop_schema::valid_account(&launcher.supermedo_npid) {
                launcher.supermedo_npid.clone()
            } else {
                Self::new_account()
            };
        }
        if config.password.is_empty() {
            config.password = if !launcher.supermedo_pass.is_empty() {
                launcher.supermedo_pass.clone()
            } else {
                Self::new_password()
            };
        }
        config
    }

    pub fn take_from_bbport_launcher(&mut self) -> bool {
        let launcher = bbport_launcher::load();
        if launcher.supermedo_npid.is_empty() && launcher.supermedo_pass.is_empty() {
            return false;
        }
        if !launcher.supermedo_npid.is_empty() {
            self.account = launcher.supermedo_npid;
        }
        if !launcher.supermedo_pass.is_empty() {
            self.password = launcher.supermedo_pass;
        }
        true
    }

    pub fn take_rules_from_bbport_launcher(&mut self) -> bool {
        let launcher = bbport_launcher::load();
        if !launcher.exists {
            return false;
        }
        self.rules = launcher.rules;
        true
    }

    pub fn save(&self) {
        let dir = folder();
        if let Err(err) = fs::create_dir_all(&dir) {
            log::line(&format!("Configuração não gravada ({err})."));
            return;
        }
        let lines = [
            "# BBRadmin: as respostas do app. Não é do bbport; pode apagar este arquivo.".to_string(),
            format!("bbport_dir={}", self.bbport_dir),
            format!("role={}", self.role),
            format!("account={}", self.account),
            format!("password={}", dpapi::protect(&self.password)),
            format!("remote_ip={}", self.remote_ip),
            format!("rule_rot={}", if self.rules.rot { "1" } else { "0" }),
            format!("rule_travel={}", if self.rules.travel { "1" } else { "0" }),
            format!("rule_vote={}", if self.rules.vote { "1" } else { "0" }),
            format!("rule_revive={}", if self.rules.revive { "1" } else { "0" }),
            format!("rule_enemies={}", self.rules.enemies),
            format!("rule_tempo={}", self.rules.tempo),
            format!("rule_custom_hp={}", self.rules.custom_hp),
            format!("rule_custom_damage={}", self.rules.custom_damage),
            format!("rule_custom_poise={}", self.rules.custom_poise),
            format!("play_with={}", self.play_with),
            format!("theme={}", self.theme.as_config()),
            format!("ui_lang={}", self.lang.as_config()),
            format!("auto_update={}", if self.auto_update { "1" } else { "0" }),
            format!(
                "auto_stop_server={}",
                if self.auto_stop_server { "1" } else { "0" }
            ),
            format!("keep_server={}", if self.auto_stop_server { "0" } else { "1" }),
            format!(
                "stop_server_with_game={}",
                if self.auto_stop_server { "1" } else { "0" }
            ),
        ];
        let body = lines.join("\r\n") + "\r\n";
        if let Err(err) = fs::write(file_path(), body.as_bytes()) {
            log::line(&format!("Configuração não gravada ({err})."));
        }
    }

    pub fn new_account() -> String {
        let user = env::var("USERNAME").unwrap_or_default();
        let clean: String = user
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        let stem = if clean.len() < 3 {
            "hunter".to_string()
        } else if clean.len() > 12 {
            clean[..12].to_string()
        } else {
            clean
        };
        let suffix: u16 = rand::thread_rng().gen_range(100..999);
        format!("{stem}-{suffix}")
    }

    pub fn new_password() -> String {
        const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut rng = rand::thread_rng();
        let mut out = String::with_capacity(16);
        for _ in 0..16 {
            let idx = rng.gen_range(0..ALPHABET.len());
            out.push(ALPHABET[idx] as char);
        }
        out
    }
}
