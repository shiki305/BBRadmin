using System;
using System.Text.RegularExpressions;

namespace BBRadmin.Core
{
    /// <summary>
    /// Every name BBRadmin depends on, in one place: bbport's files, the co-op server's ports and the
    /// environment the seamless engine reads.
    ///
    /// Why here: a bbport / BBSC: Yharnam Together update that renames any of these is one edit in this
    /// file (README, "Quando o bbport muda"). Rest of the code never spells a name out.
    /// </summary>
    internal static class CoopSchema
    {
        // ---- bbport's own files, relative to the install folder (the one with Bloodborne.exe) ----
        public const string GameExe = "Bloodborne.exe";
        public const string EngineSeamless = @"bin\bb-probe-seamless.exe";
        public const string CoopUserDir = @"seamless\user";
        public const string CoopGameView = @"seamless\coop-game\CUSA03173";
        public const string CoopModsView = @"seamless\mods-view";
        public const string CoopModsConfig = @"seamless\mods.json";
        public const string CoopEventTool = @"seamless\tools\party_boss_events.py";
        public const string CoopReleaseFile = @"seamless\VERSION.txt";
        public const string OverridesFile = @"seamless\user\host_overrides.custom.json";
        public const string InputFile = "input.ini";          // the controls file (shadPS4 syntax)
        public const string TitleId = "CUSA03173";

        // ---- the co-op server BBRadmin runs on the host, unpacked under %LOCALAPPDATA%\BBRadmin ----
        //      (BBSC: Yharnam Together 1.0.0's own Windows server; see ServerAssets for where it comes from)
        public const string ServerFolder = "server";
        public const string ServerExe = "shadnet.exe";
        public const string ServerConfig = "shadnet.cfg";
        public const string ServerLogFolder = "log";
        public const string ServerLog = "shadnet.txt";
        public const string ServerPrevLog = "shadnet.prev.txt";
        public const string BackendFolder = "backend";
        public const string BackendNotice = "notice.txt";
        public const string ServerStamp = "server-version.txt";

        // Every file the server needs: shadnet.exe is linked against these Qt libraries, and Qt loads the
        // plugins under sqldrivers / tls / networkinformation at runtime. Without the SQLite driver
        // (sqldrivers\qsqlite.dll) shadnet itself fails: "can not load requested driver 'QSQLITE'".
        public static readonly string[] ServerFiles =
        {
            "shadnet.exe", "shadnet.cfg", "Qt6Core.dll", "Qt6Network.dll",
            "Qt6HttpServer.dll", "Qt6Sql.dll", "Qt6WebSockets.dll", "icuuc.dll",
            @"sqldrivers\qsqlite.dll", @"tls\qschannelbackend.dll",
            @"networkinformation\qnetworklistmanager.dll"
        };

        // ---- the co-op server's ports (BBSC 1.0.0: bbport_yharnam.LINK_PORT/WEBAPI_PORT/BACKEND_PORT) ----
        public const int LinkPort = 31314;    // UDP: the game's link to the server
        public const int WebApiPort = 31315;  // TCP: summons and the WebAPI (/status)
        public const int BackendPort = 20443; // TCP: the game's own server (the ss.info the game asks for)
        public const int RpcPort = 31313;     // TCP: the server's own listener (the host's PC, local)

        // ---- what the co-op start is (BBSC 1.0.0, bbport_yharnam.VERSION) ----
        public const string ReleaseVersion = "1.0.0";
        public const string FixFps = "60";    // co-op is the fixed 60 FPS mode
        // The game's own server, as the port sees it (bbport_yharnam.GAME_SERVER): this is the key the
        // overrides file sends to the host's backend, so the game never asks Sony's server.
        public const string GameServer = "https://ss4.scej-network.jp:20443";

        // ---- the environment the seamless engine reads (bbport_yharnam.environment) ----
        public const string EnvProbe = "BB_PROBE";
        public const string EnvDataDir = "BB_DATA_DIR";
        public const string EnvGameDir = "BB_GAME_DIR";
        public const string EnvUserDir = "BB_USER_DIR";
        public const string EnvGpuUserDir = "BB_GPU_USER_DIR";
        public const string EnvFps = "BB_FPS";
        public const string EnvOnline = "BB_ONLINE";
        public const string EnvServer = "BB_SHADNET_SERVER";
        public const string EnvWebApi = "BB_SHADNET_WEBAPI";
        public const string EnvNpid = "BB_SHADNET_NPID";
        public const string EnvPassword = "BB_SHADNET_PASSWORD";
        public const string EnvServerKey = "BB_SHADNET_SERVER_KEY";
        public const string EnvReleaseVersion = "BB_RELEASE_VERSION";
        public const string EnvModsEnabled = "BB_MODS_ENABLED";
        public const string EnvModsDir = "BB_MODS_DIR";
        public const string EnvModsConfig = "BB_MODS_CONFIG";
        public const string EnvModsGameLast = "BB_MODS_GAME_LAST";
        public const string EnvSeamlessCoop = "SHADPS4_BLOODBORNE_SEAMLESS_COOP";
        public const string EnvReTrace = "SHADPS4_BLOODBORNE_RE_TRACE";
        public const string EnvHostRules = "SHADPS4_BLOODBORNE_HOST_RULES";
        public const string EnvHarderEnemies = "SHADPS4_BLOODBORNE_HARDER_ENEMIES";
        public const string EnvDiscordStatus = "BBSC_DISCORD_STATUS";

        // ---- the normal (non co-op) online start's variables: a co-op start drops them, as BBSC does
        //      (bbport_yharnam.NORMAL_ONLINE / NORMAL_ONLINE_PREFIXES), so none of the launcher's own
        //      online settings leaks into the co-op game ----
        public static readonly string[] NormalOnlineKeys = { "BB_ONLINE", "BB_COMMUNITY_SERVER", "BB_UPNP", "BB_PROBE" };
        public static readonly string[] NormalOnlinePrefixes = { "BB_SHADNET_", "BBSC_", "SHADPS4_" };

        // ---- variables a co-op start drops although they are not online ones: each would undo a choice
        //      the co-op makes itself (YT's game_environment pops the same three, and run.py reads
        //      bbport.ini for them only during an in-game restart, which BB_SHARED_GRAPHICS_CONFIG marks) ----
        public static readonly string[] DroppedKeys = { "BB_SHARED_GRAPHICS_CONFIG", "BB_ONLINE_UNLOCKED_FPS", "BB_FPS_LIMIT" };

        // ---- the server's own environment (bbport_coop.server_environment) ----
        public const string EnvBackendAddress = "SHADNET_BACKEND_ADDRESS";
        public const string EnvQtStderrLogging = "QT_FORCE_STDERR_LOGGING";
        public const string EnvQtLoggingRules = "QT_LOGGING_RULES";

        // ---- a player id (the co-op account): the server's own rule (shadNet's PasswordAccounts.CheckName:
        //      3 to 20 of ASCII letters, digits, '_', '-' and '.'), so an account made in the launcher's
        //      Seamless co-op page is accepted here as it is. The server makes the account at the first
        //      sign-in (its own MakeAccountsAtSignIn), so both players can keep their own ids ----
        private static readonly Regex AccountPattern = new Regex("^[A-Za-z0-9_.-]{3,20}$");

        public static bool ValidAccount(string value)
        {
            return !string.IsNullOrEmpty(value) && AccountPattern.IsMatch(value);
        }
    }
}
