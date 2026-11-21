//! Builds the full co-op game environment and starts `Bloodborne.exe --run`.
//!
//! Unlike the original C# version, this forwards display and launcher preferences from
//! `%APPDATA%\bbport-launcher\settings.json` (including `BB_FULLSCREEN`, `BB_WINDOW_MODE`,
//! `BB_HDR`, `BB_PRESENT_MODE`, `BB_LANGUAGE`, `BB_VOICE_LANGUAGE`, `BB_MENU_KEY`, `BB_MENU_PAD`,
//! etc.) and resolves the `WindowPlacer` configuration for borderless fullscreen / monitor placement.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command};

use crate::core::bbport::{self, BbportInfo};
use crate::core::bbport_launcher::{self, LauncherSettings};
use crate::core::coop_schema;
use crate::core::display::{self, ResolvedDisplay};
use crate::core::host_rules::HostRules;
use crate::core::log;
use crate::core::overrides;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct PlayPlan {
    pub env: BTreeMap<String, String>,
    pub overrides_path: String,
    pub overrides_text: String,
    pub host_ip: String,
    pub display: ResolvedDisplay,
    pub warnings: Vec<String>,
}

impl PlayPlan {
    pub fn report(&self, reveal_password: bool) -> String {
        let mut text = String::new();
        text.push_str(&format!("Endereço do servidor: {}\n", self.host_ip));
        text.push_str(&format!("Modo de tela: {}\n", self.display.describe()));
        text.push_str(&format!("Overrides: {}\n\n", self.overrides_path));
        text.push_str(&format!("--- {} ---\n", coop_schema::OVERRIDES_FILE));
        text.push_str(&self.overrides_text);
        text.push_str("\n--- variáveis do jogo ---\n");

        for (key, value) in &self.env {
            let shown = if !reveal_password && key.eq_ignore_ascii_case(coop_schema::ENV_PASSWORD) {
                "••••••••••••"
            } else {
                value.as_str()
            };
            if is_relevant_report_key(key) {
                text.push_str(&format!("  {key}={shown}\n"));
            }
        }

        if !self.warnings.is_empty() {
            text.push_str("\n--- avisos ---\n");
            for w in &self.warnings {
                text.push_str(&format!("  {w}\n"));
            }
        }
        text
    }
}

fn is_relevant_report_key(key: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "BB_SHADNET_",
        "BB_MODS",
        "BB_PROBE",
        "BB_GAME",
        "BB_USER",
        "BB_GPU",
        "BB_FPS",
        "BB_ONLINE",
        "BB_LOG",
        "BB_RELEASE",
        "BB_FULLSCREEN",
        "BB_WINDOW",
        "BB_HDR",
        "BB_PRESENT",
        "BB_LANGUAGE",
        "BB_VOICE",
        "BB_MENU",
        "BB_DISCORD",
        "BB_LOW_",
        "BB_VRR",
        "SHADPS4",
        "BBSC",
    ];
    PREFIXES.iter().any(|p| key.starts_with(p))
}

pub fn build(
    port: &BbportInfo,
    host_ip: &str,
    account: &str,
    password: &str,
    rules: &HostRules,
) -> PlayPlan {
    let launcher = bbport_launcher::load();
    build_with_launcher(port, host_ip, account, password, rules, &launcher)
}

pub fn build_with_launcher(
    port: &BbportInfo,
    host_ip: &str,
    account: &str,
    password: &str,
    rules: &HostRules,
    launcher: &LauncherSettings,
) -> PlayPlan {
    let mut env_map = BTreeMap::new();

    // Start from current process env, filtering out normal online and dropped keys.
    for (k, v) in env::vars() {
        if is_normal_online_key(&k) || is_dropped_key(&k) {
            continue;
        }
        env_map.insert(k, v);
    }

    let output_res = bbport::read_output_res(port);
    let resolved_display = display::resolve(launcher, output_res);

    // ---- 1. Forward bbport launcher settings (mirrors bbport_launcher_win.game_environment) ----
    env_map.insert(
        coop_schema::ENV_FULLSCREEN.to_string(),
        if resolved_display.bb_fullscreen { "1" } else { "0" }.to_string(),
    );
    if resolved_display.bb_fullscreen && resolved_display.exclusive {
        env_map.insert(
            coop_schema::ENV_WINDOW_MODE.to_string(),
            "fullscreen".to_string(),
        );
    } else {
        env_map.remove(coop_schema::ENV_WINDOW_MODE);
    }

    if launcher.exists {
        if !launcher.present_mode.is_empty() {
            env_map.insert(
                coop_schema::ENV_PRESENT_MODE.to_string(),
                launcher.present_mode.clone(),
            );
        }
        if launcher.hdr {
            env_map.insert(coop_schema::ENV_HDR.to_string(), "1".to_string());
        }
        if !launcher.language.is_empty() {
            env_map.insert(
                coop_schema::ENV_LANGUAGE.to_string(),
                launcher.language.clone(),
            );
        }
        if !launcher.voice_language.is_empty() {
            env_map.insert(
                coop_schema::ENV_VOICE_LANGUAGE.to_string(),
                launcher.voice_language.clone(),
            );
        }
        if !launcher.ui_language.is_empty() {
            env_map.insert(
                coop_schema::ENV_UI_LANGUAGE.to_string(),
                launcher.ui_language.clone(),
            );
        }
        if !launcher.player_name.is_empty() {
            env_map.insert(
                coop_schema::ENV_USER_NAME.to_string(),
                launcher.player_name.clone(),
            );
        }
        if !launcher.menu_key.is_empty() {
            env_map.insert(
                coop_schema::ENV_MENU_KEY.to_string(),
                launcher.menu_key.clone(),
            );
        }
        if !launcher.menu_pad.is_empty() {
            env_map.insert(
                coop_schema::ENV_MENU_PAD.to_string(),
                launcher.menu_pad.clone(),
            );
        }
        env_map.insert(
            coop_schema::ENV_DISCORD.to_string(),
            if launcher.discord_status { "1" } else { "0" }.to_string(),
        );
        env_map.insert(
            coop_schema::ENV_DISCORD_STATS.to_string(),
            if launcher.discord_stats { "1" } else { "0" }.to_string(),
        );
        if !launcher.dlc_dir.is_empty() {
            env_map.insert(coop_schema::ENV_DLC_DIR.to_string(), launcher.dlc_dir.clone());
        }
        if !launcher.patches_dir.is_empty() {
            env_map.insert(
                coop_schema::ENV_PATCHES_DIR.to_string(),
                launcher.patches_dir.clone(),
            );
        } else {
            env_map.insert(
                coop_schema::ENV_PATCHES_DIR.to_string(),
                Path::new(&port.root).join("patches").to_string_lossy().to_string(),
            );
        }
        env_map.insert(
            coop_schema::ENV_PATCHES_CONFIG.to_string(),
            Path::new(&port.root)
                .join("patches.json")
                .to_string_lossy()
                .to_string(),
        );
        env_map.insert(
            coop_schema::ENV_EXIT_BACKUP.to_string(),
            if launcher.exit_backup { "1" } else { "0" }.to_string(),
        );
        if launcher.save_backups {
            env_map.insert(coop_schema::ENV_SAVE_BACKUPS.to_string(), "1".to_string());
        }
        if !launcher.vram_warning {
            env_map.insert(coop_schema::ENV_VRAM_WARN_MB.to_string(), "0".to_string());
        }
        for (val, env_key) in [
            (&launcher.draw_pipe, "BB_DRAW_PIPE"),
            (&launcher.readbacks, "BB_READBACKS"),
            (&launcher.frames_ahead, "BB_FRAMES_AHEAD"),
            (&launcher.low_vram, "BB_LOW_VRAM"),
            (&launcher.low_latency, "BB_LOW_LATENCY"),
            (&launcher.async_pipelines, "BB_ASYNC_PIPELINES"),
            (&launcher.vrr, "BB_VRR"),
            (&launcher.early_submit, "BB_EARLY_SUBMIT"),
        ] {
            if !val.is_empty() {
                env_map.insert(env_key.to_string(), val.clone());
            }
        }
        if launcher.frame_stats {
            env_map.insert("BB_FRAME_STATS".to_string(), "1".to_string());
        }
        if launcher.gpu_profile {
            env_map.insert("BB_GPU_PROFILE".to_string(), "1".to_string());
        }
        if launcher.vk_validation {
            env_map.insert("BB_VK_VALIDATION".to_string(), "1".to_string());
        }
        for item in launcher.extra_env.split_whitespace() {
            if let Some((k, v)) = item.split_once('=') {
                if !k.is_empty() && !is_normal_online_key(k) && !is_dropped_key(k) {
                    env_map.insert(k.to_string(), v.to_string());
                }
            }
        }
    }
    env_map.insert(
        coop_schema::ENV_DISABLE_FOSSILIZE.to_string(),
        "1".to_string(),
    );

    // ---- 2. Overlay the BBSC Seamless Co-op environment (mirrors bbport_yharnam.environment) ----
    let root = Path::new(&port.root);
    let seamless = root.join(coop_schema::COOP_USER_DIR);
    let seamless_str = seamless.to_string_lossy().to_string();

    env_map.insert(
        coop_schema::ENV_PROBE.to_string(),
        root.join(coop_schema::ENGINE_SEAMLESS)
            .to_string_lossy()
            .to_string(),
    );
    env_map.insert(coop_schema::ENV_DATA_DIR.to_string(), port.root.clone());
    env_map.insert(
        coop_schema::ENV_GAME_DIR.to_string(),
        root.join(coop_schema::COOP_GAME_VIEW)
            .to_string_lossy()
            .to_string(),
    );
    env_map.insert(coop_schema::ENV_USER_DIR.to_string(), seamless_str.clone());
    env_map.insert(coop_schema::ENV_GPU_USER_DIR.to_string(), seamless_str);
    env_map.insert(
        coop_schema::ENV_FPS.to_string(),
        coop_schema::FIX_FPS.to_string(),
    );
    env_map.insert(coop_schema::ENV_ONLINE.to_string(), "1".to_string());
    env_map.insert(
        coop_schema::ENV_SERVER.to_string(),
        format!("{host_ip}:{}", coop_schema::LINK_PORT),
    );
    env_map.insert(
        coop_schema::ENV_WEBAPI.to_string(),
        format!("http://{host_ip}:{}", coop_schema::WEBAPI_PORT),
    );
    env_map.insert(coop_schema::ENV_NPID.to_string(), account.to_string());
    env_map.insert(coop_schema::ENV_PASSWORD.to_string(), password.to_string());
    env_map.insert(
        coop_schema::ENV_RELEASE_VERSION.to_string(),
        coop_schema::RELEASE_VERSION.to_string(),
    );
    env_map.insert(coop_schema::ENV_MODS_ENABLED.to_string(), "1".to_string());
    env_map.insert(
        coop_schema::ENV_MODS_DIR.to_string(),
        root.join(coop_schema::COOP_MODS_VIEW)
            .to_string_lossy()
            .to_string(),
    );
    env_map.insert(
        coop_schema::ENV_MODS_CONFIG.to_string(),
        root.join(coop_schema::COOP_MODS_CONFIG)
            .to_string_lossy()
            .to_string(),
    );
    env_map.insert(coop_schema::ENV_MODS_GAME_LAST.to_string(), "1".to_string());
    env_map.insert(coop_schema::ENV_SEAMLESS_COOP.to_string(), "1".to_string());
    env_map.insert(coop_schema::ENV_RE_TRACE.to_string(), "0".to_string());
    env_map.insert(coop_schema::ENV_DISCORD_STATUS.to_string(), "0".to_string());

    let rules_text = rules.text();
    if !rules_text.is_empty() {
        env_map.insert(coop_schema::ENV_HOST_RULES.to_string(), rules_text);
    }
    let harder = rules.enemies_text();
    if !harder.is_empty() {
        env_map.insert(coop_schema::ENV_HARDER_ENEMIES.to_string(), harder);
    }

    PlayPlan {
        env: env_map,
        overrides_path: overrides::path_in(port).to_string_lossy().to_string(),
        overrides_text: overrides::text(host_ip),
        host_ip: host_ip.to_string(),
        display: resolved_display,
        warnings: port.notes.clone(),
    }
}

pub fn is_normal_online_key(key: &str) -> bool {
    if coop_schema::NORMAL_ONLINE_KEYS
        .iter()
        .any(|n| key.eq_ignore_ascii_case(n))
    {
        return true;
    }
    let upper = key.to_ascii_uppercase();
    coop_schema::NORMAL_ONLINE_PREFIXES
        .iter()
        .any(|p| upper.starts_with(p))
}

fn is_dropped_key(key: &str) -> bool {
    coop_schema::DROPPED_KEYS
        .iter()
        .any(|n| key.eq_ignore_ascii_case(n))
}

pub fn start(port: &BbportInfo, plan: &PlayPlan) -> Result<Child, String> {
    let root = Path::new(&port.root);
    let exe = root.join(coop_schema::GAME_EXE);
    if !exe.is_file() {
        return Err(format!("{} não está em {}", coop_schema::GAME_EXE, port.root));
    }

    let view = root.join(coop_schema::COOP_GAME_VIEW);
    if !view.join("eboot.bin").is_file() {
        return Err(format!(
            "O jogo do co-op ainda não foi preparado.\n\nAbra o launcher do bbport uma vez, entre em \"Seamless co-op\" e deixe ele preparar; depois volte aqui.\n\nFaltando: {}\\eboot.bin",
            coop_schema::COOP_GAME_VIEW
        ));
    }

    let seamless_log = root.join(coop_schema::COOP_USER_DIR).join("log");
    let _ = fs::create_dir_all(&seamless_log);

    let child = Command::new(&exe)
        .arg("--run")
        .current_dir(root)
        .env_clear()
        .envs(&plan.env)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Falha ao iniciar {}: {e}", exe.display()))?;

    log::line(&format!(
        "Jogo iniciado: {} --run (servidor {}, tela: {}).",
        exe.display(),
        plan.host_ip,
        plan.display.describe()
    ));
    Ok(child)
}
