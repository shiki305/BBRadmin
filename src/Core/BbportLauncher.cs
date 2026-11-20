using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace BBRadmin.Core
{
    /// <summary>
    /// The bbport launcher's own settings (%APPDATA%\bbport-launcher\settings.json), read only.
    ///
    /// Why BBRadmin reads it: BBRadmin starts the game itself, with the co-op environment the port would
    /// build (bbport_yharnam.environment). Two things in that environment come from the launcher's own
    /// settings, so they have to be read from here or the co-op starts without them:
    ///
    /// - the SuperMedo account the co-op signs in with (`supermedo_npid` / `supermedo_pass`): the player
    ///   made it once on the launcher's Seamless co-op page;
    /// - the host rules chosen on that same page (`yt_*`), which the in-game team screen shows as the
    ///   leader's and which the launcher sends as SHADPS4_BLOODBORNE_HOST_RULES / _HARDER_ENEMIES. These
    ///   are only the default: BBRadmin's own window edits the same rules, and its own choices win.
    ///
    /// Nothing of the file is ever written; a missing or unreadable one is simply no values.
    /// </summary>
    internal static class BbportLauncher
    {
        public static string SettingsPath
        {
            get
            {
                return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                                    "bbport-launcher", "settings.json");
            }
        }

        /// <summary>The SuperMedo account's id the launcher signed in with ('' when there is none).</summary>
        public static string Account()
        {
            return Text("supermedo_npid").Trim();
        }

        /// <summary>The value the launcher keeps as the account's password ('' when there is none).</summary>
        public static string Password()
        {
            return Text("supermedo_pass");
        }

        /// <summary>
        /// What the launcher's Controls page says the player plays with: "controller", "keyboard and
        /// mouse", or '' when it says nothing. Only the starting value of BBRadmin's own switch.
        /// </summary>
        public static string PlayWith()
        {
            string value = Text("play_with").Trim().ToLowerInvariant();
            if (value == InputConfig.Keyboard || value == InputConfig.Controller) return value;
            return "";
        }

        /// <summary>
        /// The leader's co-op rules as the launcher's Seamless co-op page has them (its yt_* keys), in the
        /// same model BBRadmin's own window edits. This is the default the player starts from; the choices
        /// BBRadmin keeps in its own config win once the player has set them there. A missing launcher is
        /// the model's own defaults.
        /// </summary>
        public static HostRules Rules()
        {
            HostRules rules = new HostRules();
            rules.Rot = Flag("yt_rot");
            rules.Travel = Flag("yt_travel");
            rules.Vote = Flag("yt_vote");
            rules.Revive = Flag("yt_revive", true);
            string enemies = Text("yt_enemies").Trim();
            if (enemies.Length > 0) rules.Enemies = enemies;
            string tempo = Text("yt_tempo").Trim();
            if (tempo.Length > 0) rules.Tempo = tempo;
            string hp = Text("yt_custom_hp").Trim();
            if (hp.Length > 0) rules.CustomHp = hp;
            string damage = Text("yt_custom_damage").Trim();
            if (damage.Length > 0) rules.CustomDamage = damage;
            string poise = Text("yt_custom_poise").Trim();
            if (poise.Length > 0) rules.CustomPoise = poise;
            return rules;
        }

        private static bool Flag(string key, bool fallback = false)
        {
            string text = Text(key).Trim().ToLowerInvariant();
            if (text == "true") return true;
            if (text == "false") return false;
            return fallback;
        }

        // The file is read once and kept: the co-op start asks for several keys, the file never changes
        // while the app runs. Only the top-level scalars matter here, and every key read above is unique
        // to the top level, so a flat scan of the text is enough (no JSON library).
        private static Dictionary<string, string> _values;

        /// <summary>Forgets the read values, so the next question reads the file again (the player may have
        /// signed in on the bbport launcher while this window was open).</summary>
        public static void Invalidate()
        {
            _values = null;
        }

        private static string Text(string key)
        {
            Dictionary<string, string> values = Values();
            string value;
            return values.TryGetValue(key, out value) ? value : "";
        }

        private static Dictionary<string, string> Values()
        {
            if (_values != null) return _values;
            Dictionary<string, string> values = new Dictionary<string, string>(StringComparer.Ordinal);
            try
            {
                if (File.Exists(SettingsPath))
                {
                    string text = File.ReadAllText(SettingsPath, Encoding.UTF8);
                    MatchCollection found = Regex.Matches(text,
                        "\"([^\"]+)\"\\s*:\\s*(?:\"((?:[^\"\\\\]|\\\\.)*)\"|(true|false)|(-?\\d+(?:\\.\\d+)?))");
                    foreach (Match match in found)
                    {
                        string key = match.Groups[1].Value;
                        if (values.ContainsKey(key)) continue;   // the first hit is the outer value
                        if (match.Groups[2].Success) values[key] = Regex.Unescape(match.Groups[2].Value);
                        else if (match.Groups[3].Success) values[key] = match.Groups[3].Value;
                        else values[key] = match.Groups[4].Value;
                    }
                }
            }
            catch (Exception error)
            {
                Log.Line("Configurações do launcher do bbport não lidas (" + error.Message + ").");
            }
            _values = values;
            return values;
        }
    }
}
