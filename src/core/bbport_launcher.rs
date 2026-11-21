//! Read-only reader for the bbport launcher's settings (%APPDATA%\bbport-launcher\settings.json).
//!
//! Reads:
//! - the SuperMedo co-op account (`supermedo_npid` / `supermedo_pass`)
//! - the leader's co-op host rules (`yt_*`)
//! - the controls preference (`play_with`)
//! - display & launcher settings (`fullscreen`, `exclusive_fullscreen`, `borderless_fullscreen`,
//!   `game_monitor`, `window_size`, `window_custom`, `window_last`, `hdr`, `present_mode`,
//!   `language`, `voice_language`, `ui_language`, `player_name`, `menu_key`, `menu_pad`,
//!   `discord_status`, `discord_stats`, `dlc_dir`, `patches_dir`, `low_latency`, `vrr`, etc.)
//!   so BBRadmin starts the game with the exact display mode and launcher settings the player configured!

use std::env;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::core::host_rules::HostRules;
use crate::core::input_config;
use crate::core::log;

#[derive(Debug, Clone)]
pub struct LauncherSettings {
    pub exists: bool,
    pub game_dir: String,
    pub supermedo_npid: String,
    pub supermedo_pass: String,
    pub play_with: String,
    pub rules: HostRules,

    // Display & window placement
    pub fullscreen: bool,
    pub exclusive_fullscreen: bool,
    pub borderless_fullscreen: bool,
    pub game_monitor: String,
    pub window_size: String,
    pub window_custom: String,
    pub window_last: String,
    pub hdr: bool,
    pub present_mode: String,

    // Gameplay / UI / Advanced launcher settings passed via environment
    pub language: String,
    pub voice_language: String,
    pub ui_language: String,
    pub player_name: String,
    pub menu_key: String,
    pub menu_pad: String,
    pub discord_status: bool,
    pub discord_stats: bool,
    pub dlc_dir: String,
    pub patches_dir: String,
    pub draw_pipe: String,
    pub readbacks: String,
    pub frames_ahead: String,
    pub low_vram: String,
    pub low_latency: String,
    pub async_pipelines: String,
    pub vrr: String,
    pub early_submit: String,
    pub vram_warning: bool,
    pub frame_stats: bool,
    pub gpu_profile: bool,
    pub vk_validation: bool,
    pub save_backups: bool,
    pub exit_backup: bool,
    pub extra_env: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            exists: false,
            game_dir: String::new(),
            supermedo_npid: String::new(),
            supermedo_pass: String::new(),
            play_with: String::new(),
            rules: HostRules::default(),
            fullscreen: false,
            exclusive_fullscreen: false,
            borderless_fullscreen: false,
            game_monitor: String::new(),
            window_size: "output".to_string(),
            window_custom: String::new(),
            window_last: String::new(),
            hdr: false,
            present_mode: String::new(),
            language: String::new(),
            voice_language: String::new(),
            ui_language: String::new(),
            player_name: String::new(),
            menu_key: "Insert".to_string(),
            menu_pad: "l3+r3".to_string(),
            discord_status: true,
            discord_stats: true,
            dlc_dir: String::new(),
            patches_dir: String::new(),
            draw_pipe: String::new(),
            readbacks: String::new(),
            frames_ahead: String::new(),
            low_vram: String::new(),
            low_latency: String::new(),
            async_pipelines: String::new(),
            vrr: String::new(),
            early_submit: String::new(),
            vram_warning: true,
            frame_stats: false,
            gpu_profile: false,
            vk_validation: false,
            save_backups: false,
            exit_backup: false,
            extra_env: String::new(),
        }
    }
}

pub fn settings_path() -> PathBuf {
    let appdata = env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
        .join("bbport-launcher")
        .join("settings.json")
}

/// Reads %APPDATA%\bbport-launcher\settings.json fresh from disk (never writes to it).
pub fn load() -> LauncherSettings {
    let mut out = LauncherSettings::default();
    let path = settings_path();
    if !path.is_file() {
        return out;
    }
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(err) => {
            log::line(&format!(
                "Configurações do launcher do bbport não lidas ({err})."
            ));
            return out;
        }
    };
    let parsed: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(err) => {
            log::line(&format!(
                "JSON do launcher do bbport inválido ({err})."
            ));
            return out;
        }
    };
    out.exists = true;

    let get_str = |k: &str| -> String {
        match parsed.get(k) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::Bool(b)) => b.to_string(),
            _ => String::new(),
        }
    };
    let get_bool = |k: &str, default: bool| -> bool {
        match parsed.get(k) {
            Some(Value::Bool(b)) => *b,
            Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
                "true" | "1" => true,
                "false" | "0" => false,
                _ => default,
            },
            _ => default,
        }
    };

    out.game_dir = get_str("game_dir");
    out.supermedo_npid = get_str("supermedo_npid").trim().to_string();
    out.supermedo_pass = get_str("supermedo_pass");

    let pw = get_str("play_with").trim().to_ascii_lowercase();
    if pw == input_config::KEYBOARD || pw == input_config::CONTROLLER {
        out.play_with = pw;
    }

    // Host rules
    out.rules.rot = get_bool("yt_rot", false);
    out.rules.travel = get_bool("yt_travel", false);
    out.rules.vote = get_bool("yt_vote", false);
    out.rules.revive = get_bool("yt_revive", true);
    let enemies = get_str("yt_enemies").trim().to_string();
    if !enemies.is_empty() {
        out.rules.enemies = enemies;
    }
    let tempo = get_str("yt_tempo").trim().to_string();
    if !tempo.is_empty() {
        out.rules.tempo = tempo;
    }
    let hp = get_str("yt_custom_hp").trim().to_string();
    if !hp.is_empty() {
        out.rules.custom_hp = hp;
    }
    let damage = get_str("yt_custom_damage").trim().to_string();
    if !damage.is_empty() {
        out.rules.custom_damage = damage;
    }
    let poise = get_str("yt_custom_poise").trim().to_string();
    if !poise.is_empty() {
        out.rules.custom_poise = poise;
    }

    // Display settings (the ones original BBRadmin forgot to read!)
    out.fullscreen = get_bool("fullscreen", false);
    out.exclusive_fullscreen = get_bool("exclusive_fullscreen", false);
    out.borderless_fullscreen = get_bool("borderless_fullscreen", false);
    out.game_monitor = get_str("game_monitor").trim().to_string();
    let ws = get_str("window_size").trim().to_string();
    if !ws.is_empty() {
        out.window_size = ws;
    }
    out.window_custom = get_str("window_custom").trim().to_string();
    out.window_last = get_str("window_last").trim().to_string();
    out.hdr = get_bool("hdr", false);
    out.present_mode = get_str("present_mode").trim().to_string();

    // Gameplay / launcher settings
    out.language = get_str("language").trim().to_string();
    out.voice_language = get_str("voice_language").trim().to_string();
    out.ui_language = get_str("ui_language").trim().to_string();
    out.player_name = get_str("player_name").trim().to_string();
    let mk = get_str("menu_key").trim().to_string();
    if !mk.is_empty() {
        out.menu_key = mk;
    }
    let mp = get_str("menu_pad").trim().to_string();
    if !mp.is_empty() {
        out.menu_pad = mp;
    }
    out.discord_status = get_bool("discord_status", true);
    out.discord_stats = get_bool("discord_stats", true);
    out.dlc_dir = get_str("dlc_dir").trim().to_string();
    out.patches_dir = get_str("patches_dir").trim().to_string();
    out.draw_pipe = get_str("draw_pipe").trim().to_string();
    out.readbacks = get_str("readbacks").trim().to_string();
    out.frames_ahead = get_str("frames_ahead").trim().to_string();
    out.low_vram = get_str("low_vram").trim().to_string();
    out.low_latency = get_str("low_latency").trim().to_string();
    out.async_pipelines = get_str("async_pipelines").trim().to_string();
    out.vrr = get_str("vrr").trim().to_string();
    out.early_submit = get_str("early_submit").trim().to_string();
    out.vram_warning = get_bool("vram_warning", true);
    out.frame_stats = get_bool("frame_stats", false);
    out.gpu_profile = get_bool("gpu_profile", false);
    out.vk_validation = get_bool("vk_validation", false);
    out.save_backups = get_bool("save_backups", false);
    out.exit_backup = get_bool("exit_backup", false);
    out.extra_env = get_str("extra_env").trim().to_string();

    out
}
