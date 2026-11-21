//! Every name BBRadmin depends on, in one place: bbport's files, the co-op server's ports, and the
//! environment variables the seamless engine and run.py read.

// ---- bbport's own files, relative to the install folder (the one with Bloodborne.exe) ----
pub const GAME_EXE: &str = "Bloodborne.exe";
pub const ENGINE_NORMAL: &str = r"bin\bb-probe.exe";
pub const ENGINE_SEAMLESS: &str = r"bin\bb-probe-seamless.exe";
pub const COOP_USER_DIR: &str = r"seamless\user";
pub const COOP_GAME_VIEW: &str = r"seamless\coop-game\CUSA03173";
pub const COOP_MODS_VIEW: &str = r"seamless\mods-view";
pub const COOP_MODS_CONFIG: &str = r"seamless\mods.json";
pub const COOP_RELEASE_FILE: &str = r"seamless\VERSION.txt";
pub const OVERRIDES_FILE: &str = r"seamless\user\host_overrides.custom.json";
pub const INPUT_FILE: &str = "input.ini";
pub const BBPORT_INI: &str = "bbport.ini";

// ---- the co-op server BBRadmin runs on the host, unpacked under %LOCALAPPDATA%\BBRadmin ----
pub const SERVER_FOLDER: &str = "server";
pub const SERVER_EXE: &str = "shadnet.exe";
pub const SERVER_LOG_FOLDER: &str = "log";
pub const SERVER_LOG: &str = "shadnet.txt";
pub const SERVER_PREV_LOG: &str = "shadnet.prev.txt";
pub const BACKEND_FOLDER: &str = "backend";
pub const BACKEND_NOTICE: &str = "notice.txt";
pub const SERVER_STAMP: &str = "server-version.txt";

pub const SERVER_FILES: &[&str] = &[
    "shadnet.exe",
    "shadnet.cfg",
    "Qt6Core.dll",
    "Qt6Network.dll",
    "Qt6HttpServer.dll",
    "Qt6Sql.dll",
    "Qt6WebSockets.dll",
    "icuuc.dll",
    r"sqldrivers\qsqlite.dll",
    r"tls\qschannelbackend.dll",
    r"networkinformation\qnetworklistmanager.dll",
];

// ---- the co-op server's ports (BBSC 1.0.0) ----
pub const LINK_PORT: u16 = 31314;    // UDP: the game's link to the server
pub const WEBAPI_PORT: u16 = 31315;  // TCP: summons and the WebAPI (/status)
pub const BACKEND_PORT: u16 = 20443; // TCP: the game's own server (ss.info)
pub const RPC_PORT: u16 = 31313;     // TCP: the server's own listener (local)

// ---- what the co-op start is (BBSC 1.0.0) ----
pub const RELEASE_VERSION: &str = "1.0.0";
pub const FIX_FPS: &str = "60";
pub const GAME_SERVER: &str = "https://ss4.scej-network.jp:20443";

// ---- the environment the seamless engine & run.py read ----
pub const ENV_PROBE: &str = "BB_PROBE";
pub const ENV_DATA_DIR: &str = "BB_DATA_DIR";
pub const ENV_GAME_DIR: &str = "BB_GAME_DIR";
pub const ENV_USER_DIR: &str = "BB_USER_DIR";
pub const ENV_GPU_USER_DIR: &str = "BB_GPU_USER_DIR";
pub const ENV_FPS: &str = "BB_FPS";
pub const ENV_ONLINE: &str = "BB_ONLINE";
pub const ENV_SERVER: &str = "BB_SHADNET_SERVER";
pub const ENV_WEBAPI: &str = "BB_SHADNET_WEBAPI";
pub const ENV_NPID: &str = "BB_SHADNET_NPID";
pub const ENV_PASSWORD: &str = "BB_SHADNET_PASSWORD";
pub const ENV_RELEASE_VERSION: &str = "BB_RELEASE_VERSION";
pub const ENV_MODS_ENABLED: &str = "BB_MODS_ENABLED";
pub const ENV_MODS_DIR: &str = "BB_MODS_DIR";
pub const ENV_MODS_CONFIG: &str = "BB_MODS_CONFIG";
pub const ENV_MODS_GAME_LAST: &str = "BB_MODS_GAME_LAST";
pub const ENV_SEAMLESS_COOP: &str = "SHADPS4_BLOODBORNE_SEAMLESS_COOP";
pub const ENV_RE_TRACE: &str = "SHADPS4_BLOODBORNE_RE_TRACE";
pub const ENV_HOST_RULES: &str = "SHADPS4_BLOODBORNE_HOST_RULES";
pub const ENV_HARDER_ENEMIES: &str = "SHADPS4_BLOODBORNE_HARDER_ENEMIES";
pub const ENV_DISCORD_STATUS: &str = "BBSC_DISCORD_STATUS";

// ---- display & launcher settings from %APPDATA%\bbport-launcher\settings.json ----
pub const ENV_FULLSCREEN: &str = "BB_FULLSCREEN";
pub const ENV_WINDOW_MODE: &str = "BB_WINDOW_MODE";
pub const ENV_PRESENT_MODE: &str = "BB_PRESENT_MODE";
pub const ENV_HDR: &str = "BB_HDR";
pub const ENV_LANGUAGE: &str = "BB_LANGUAGE";
pub const ENV_VOICE_LANGUAGE: &str = "BB_VOICE_LANGUAGE";
pub const ENV_UI_LANGUAGE: &str = "BB_UI_LANGUAGE";
pub const ENV_USER_NAME: &str = "BB_USER_NAME";
pub const ENV_MENU_KEY: &str = "BB_MENU_KEY";
pub const ENV_MENU_PAD: &str = "BB_MENU_PAD";
pub const ENV_DISCORD: &str = "BB_DISCORD";
pub const ENV_DISCORD_STATS: &str = "BB_DISCORD_STATS";
pub const ENV_DLC_DIR: &str = "BB_DLC_DIR";
pub const ENV_PATCHES_DIR: &str = "BB_PATCHES_DIR";
pub const ENV_PATCHES_CONFIG: &str = "BB_PATCHES_CONFIG";
pub const ENV_EXIT_BACKUP: &str = "BB_EXIT_BACKUP";
pub const ENV_SAVE_BACKUPS: &str = "BB_SAVE_BACKUPS";
pub const ENV_VRAM_WARN_MB: &str = "BB_VRAM_WARN_MB";
pub const ENV_DISABLE_FOSSILIZE: &str = "DISABLE_VK_LAYER_VALVE_steam_fossilize_1";

// ---- normal online variables dropped before building co-op env ----
pub const NORMAL_ONLINE_KEYS: &[&str] = &[
    "BB_ONLINE",
    "BB_COMMUNITY_SERVER",
    "BB_UPNP",
    "BB_PROBE",
];
pub const NORMAL_ONLINE_PREFIXES: &[&str] = &["BB_SHADNET_", "BBSC_", "SHADPS4_"];

// ---- variables dropped because co-op forces 60 FPS and avoids shared-config override ----
pub const DROPPED_KEYS: &[&str] = &[
    "BB_SHARED_GRAPHICS_CONFIG",
    "BB_ONLINE_UNLOCKED_FPS",
    "BB_FPS_LIMIT",
];

// ---- the server's own environment ----
pub const ENV_BACKEND_ADDRESS: &str = "SHADNET_BACKEND_ADDRESS";
pub const ENV_QT_STDERR_LOGGING: &str = "QT_FORCE_STDERR_LOGGING";
pub const ENV_QT_LOGGING_RULES: &str = "QT_LOGGING_RULES";

/// Matches shadNet's PasswordAccounts.CheckName: 3 to 20 of ASCII letters, digits, '_', '-' and '.'.
pub fn valid_account(value: &str) -> bool {
    let len = value.len();
    if !(3..=20).contains(&len) {
        return false;
    }
    value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_validation() {
        assert!(valid_account("anreren"));
        assert!(valid_account("hunter-123"));
        assert!(valid_account("a_b.c"));
        assert!(!valid_account("ab"));
        assert!(!valid_account("abcdefghijklmnopqrstu"));
        assert!(!valid_account("bad name"));
    }
}
