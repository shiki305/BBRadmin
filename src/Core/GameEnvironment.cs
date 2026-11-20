using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>Everything a co-op start is: the environment, the file it writes and what to warn about.</summary>
    internal sealed class PlayPlan
    {
        public Dictionary<string, string> Env = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        public string OverridesPath = "";
        public string OverridesText = "";
        public string HostIp = "";
        public List<string> Warnings = new List<string>();

        public string Report(bool revealPassword)
        {
            StringBuilder text = new StringBuilder();
            text.AppendLine("Endereço do servidor: " + HostIp);
            text.AppendLine("Overrides: " + OverridesPath);
            text.AppendLine();
            text.AppendLine("--- " + CoopSchema.OverridesFile + " ---");
            text.Append(OverridesText);
            text.AppendLine();
            text.AppendLine("--- variáveis do jogo ---");
            List<string> keys = new List<string>(Env.Keys);
            keys.Sort(StringComparer.Ordinal);
            foreach (string key in keys)
            {
                string value = Env[key];
                if (!revealPassword && key == CoopSchema.EnvPassword) value = "••••••••••••";
                if (key.StartsWith("BB_SHADNET_") || key.StartsWith("BB_MODS") || key.StartsWith("BB_PROBE") ||
                    key.StartsWith("BB_GAME") || key.StartsWith("BB_USER") || key.StartsWith("BB_GPU") ||
                    key.StartsWith("BB_FPS") || key.StartsWith("BB_ONLINE") || key.StartsWith("BB_LOG") ||
                    key.StartsWith("BB_RELEASE") || key.StartsWith("SHADPS4") || key.StartsWith("BBSC"))
                {
                    text.AppendLine("  " + key + "=" + value);
                }
            }
            if (Warnings.Count > 0)
            {
                text.AppendLine();
                text.AppendLine("--- avisos ---");
                foreach (string warning in Warnings) text.AppendLine("  " + warning);
            }
            return text.ToString();
        }
    }

    /// <summary>
    /// The co-op start itself: Yharnam Together 1.0.0's own environment (bbport_yharnam.environment),
    /// pointed at the host instead of seamless.supermedo.com. The game is the port's own Bloodborne.exe,
    /// started as "Bloodborne.exe --run" with this environment, so its engine is
    /// bin\bb-probe-seamless.exe — this port's engine with the co-op — and the launcher's normal online
    /// settings are never touched.
    ///
    /// What the environment does NOT carry, deliberately: BB_FPS comes from here (the co-op is the fixed
    /// 60 FPS mode, YT's game_environment), while the rest of the picture (resolution, upscaler, patches,
    /// mods) stays the player's own, read from bbport.ini by run.py and the engine.
    /// </summary>
    internal static class GameEnvironment
    {
        public static PlayPlan Build(BbportInfo port, string hostIp, string account, string password, HostRules rules)
        {
            PlayPlan plan = new PlayPlan();
            plan.HostIp = hostIp;
            plan.OverridesText = Overrides.Text(hostIp);
            plan.OverridesPath = Overrides.PathIn(port);

            // The environment the port would build, without the normal online keys (YT drops the same
            // ones: bbport_yharnam.NORMAL_ONLINE) and without the three a co-op start decides by itself.
            foreach (DictionaryEntry entry in Environment.GetEnvironmentVariables())
            {
                string key = (string)entry.Key;
                if (IsNormalOnlineKey(key) || IsDroppedKey(key)) continue;
                plan.Env[key] = (string)entry.Value;
            }

            string seamless = Path.Combine(port.Root, CoopSchema.CoopUserDir);
            plan.Env[CoopSchema.EnvProbe] = Path.Combine(port.Root, CoopSchema.EngineSeamless);
            plan.Env[CoopSchema.EnvDataDir] = port.Root;          // bbport.ini, patches, mods: the port's own
            plan.Env[CoopSchema.EnvGameDir] = Path.Combine(port.Root, CoopSchema.CoopGameView);
            plan.Env[CoopSchema.EnvUserDir] = seamless;           // the co-op characters' own saves
            plan.Env[CoopSchema.EnvGpuUserDir] = seamless;        // its caches stay beside them
            plan.Env[CoopSchema.EnvFps] = CoopSchema.FixFps;
            plan.Env[CoopSchema.EnvOnline] = "1";
            plan.Env[CoopSchema.EnvServer] = hostIp + ":" + CoopSchema.LinkPort.ToString(System.Globalization.CultureInfo.InvariantCulture);
            plan.Env[CoopSchema.EnvWebApi] = "http://" + hostIp + ":" + CoopSchema.WebApiPort.ToString(System.Globalization.CultureInfo.InvariantCulture);
            plan.Env[CoopSchema.EnvNpid] = account;
            plan.Env[CoopSchema.EnvPassword] = password;
            plan.Env[CoopSchema.EnvReleaseVersion] = CoopSchema.ReleaseVersion;
            plan.Env[CoopSchema.EnvModsEnabled] = "1";
            plan.Env[CoopSchema.EnvModsDir] = Path.Combine(port.Root, CoopSchema.CoopModsView);
            plan.Env[CoopSchema.EnvModsConfig] = Path.Combine(port.Root, CoopSchema.CoopModsConfig);
            plan.Env[CoopSchema.EnvModsGameLast] = "1";
            plan.Env[CoopSchema.EnvSeamlessCoop] = "1";
            plan.Env[CoopSchema.EnvReTrace] = "0";
            plan.Env[CoopSchema.EnvDiscordStatus] = "0";

            // The leader's host rules and harder enemies (bbport_yharnam.host_rules / custom_enemies). The
            // port's launcher sends them in the environment; the in-game team screen reads them from the
            // leader's start and shows them to everyone, so a start without them leaves the team screen
            // with no rules to show. Empty means none, and is left out exactly as the port leaves it out.
            string rulesText = rules.Text();
            if (rulesText.Length > 0) plan.Env[CoopSchema.EnvHostRules] = rulesText;
            string harder = rules.EnemiesText();
            if (harder.Length > 0) plan.Env[CoopSchema.EnvHarderEnemies] = harder;

            foreach (string note in port.Notes) plan.Warnings.Add(note);
            return plan;
        }

        public static bool IsNormalOnlineKey(string key)
        {
            foreach (string name in CoopSchema.NormalOnlineKeys)
            {
                if (string.Equals(key, name, StringComparison.OrdinalIgnoreCase)) return true;
            }
            foreach (string prefix in CoopSchema.NormalOnlinePrefixes)
            {
                if (key.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static bool IsDroppedKey(string key)
        {
            foreach (string name in CoopSchema.DroppedKeys)
            {
                if (string.Equals(key, name, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        /// <summary>
        /// Starts "Bloodborne.exe --run" with this plan's environment. Throws when the game view or the
        /// engine is not there: run.py would fail on them further on with a message about eboot.bin or the
        /// saves folder, which says nothing about what the player has to do.
        /// </summary>
        public static Process Start(BbportInfo port, PlayPlan plan)
        {
            string exe = Path.Combine(port.Root, CoopSchema.GameExe);
            if (!File.Exists(exe)) throw new FileNotFoundException(CoopSchema.GameExe + " não está em " + port.Root, exe);

            string view = Path.Combine(port.Root, CoopSchema.CoopGameView);
            if (!File.Exists(Path.Combine(view, "eboot.bin")))
            {
                throw new FileNotFoundException(
                    "O jogo do co-op ainda não foi preparado.\n\nAbra o launcher do bbport uma vez, entre em " +
                    "\"Seamless co-op\" e deixe ele preparar; depois volte aqui.\n\nFaltando: " +
                    CoopSchema.CoopGameView + "\\eboot.bin", view);
            }

            // run.py refuses to start when the saves folder it is given does not exist.
            string seamless = Path.Combine(port.Root, CoopSchema.CoopUserDir);
            Directory.CreateDirectory(Path.Combine(seamless, "log"));

            ProcessStartInfo start = new ProcessStartInfo(exe);
            start.Arguments = "--run";
            start.WorkingDirectory = port.Root;
            start.UseShellExecute = false;
            // Only the environment built above: an inherited BB_SHADNET_* or SHADPS4_* would win over the
            // co-op's (run.py reads the process environment).
            start.EnvironmentVariables.Clear();
            foreach (KeyValuePair<string, string> pair in plan.Env) start.EnvironmentVariables[pair.Key] = pair.Value;
            Process process = Process.Start(start);
            Log.Line("Jogo iniciado: " + exe + " --run (servidor " + plan.HostIp + ").");
            return process;
        }
    }
}
