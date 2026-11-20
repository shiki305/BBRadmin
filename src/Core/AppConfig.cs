using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>
    /// BBRadmin's own settings, in %LOCALAPPDATA%\BBRadmin\config.ini (a plain key=value file: no JSON
    /// library is needed and nothing of the player's is touched). The same folder holds the co-op server
    /// BBRadmin unpacks and its log. The co-op password is kept with DPAPI for the current user, so it is
    /// not readable from the file by another account.
    /// </summary>
    internal sealed class AppConfig
    {
        public const string RoleHost = "host";
        public const string RoleGuest = "guest";

        public string BbportDir = "";
        public string Role = RoleHost;
        public string Account = "";
        public string Password = "";
        public string RemoteIp = "";
        public HostRules Rules = new HostRules();  // the leader's co-op rules (only a host's matter)
        public string PlayWith = "";            // controls: InputConfig.Keyboard/Controller, '' = don't touch
        public bool KeepServer = false;         // leave the server running when the app closes
        public bool StopServerWithGame = true;  // stop the server we started when the game ends

        public static string Folder
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "BBRadmin"); }
        }

        public static string FilePath
        {
            get { return Path.Combine(Folder, "config.ini"); }
        }

        public static AppConfig Load()
        {
            AppConfig config = new AppConfig();
            // The rules start from the launcher's own Seamless co-op page (BbportLauncher), then whatever
            // the player set in this window wins. A first run therefore already has the launcher's rules.
            config.Rules = BbportLauncher.Rules();
            // The controls the launcher's Controls page says the player plays with (the mouse camera on
            // or off): only the starting value; this window's own switch wins once it has been used.
            config.PlayWith = BbportLauncher.PlayWith();
            try
            {
                // A missing file is not an early exit: the account and password below must be filled in
                // either way (a first run reads them from the bbport launcher).
                foreach (string raw in (File.Exists(FilePath) ? File.ReadAllLines(FilePath, Encoding.UTF8) : new string[0]))
                {
                    string line = raw.Trim();
                    if (line.Length == 0 || line.StartsWith("#")) continue;
                    int split = line.IndexOf('=');
                    if (split <= 0) continue;
                    string key = line.Substring(0, split).Trim();
                    string value = line.Substring(split + 1).Trim();
                    if (key == "bbport_dir") config.BbportDir = value;
                    else if (key == "role") config.Role = (value == RoleGuest ? RoleGuest : RoleHost);
                    else if (key == "account") config.Account = value;
                    else if (key == "password") config.Password = Unprotect(value);
                    else if (key == "remote_ip") config.RemoteIp = value;
                    else if (key == "rule_rot") config.Rules.Rot = (value == "1");
                    else if (key == "rule_travel") config.Rules.Travel = (value == "1");
                    else if (key == "rule_vote") config.Rules.Vote = (value == "1");
                    else if (key == "rule_revive") config.Rules.Revive = (value != "0");
                    else if (key == "rule_enemies") config.Rules.Enemies = value;
                    else if (key == "rule_tempo") config.Rules.Tempo = value;
                    else if (key == "rule_custom_hp") config.Rules.CustomHp = value;
                    else if (key == "rule_custom_damage") config.Rules.CustomDamage = value;
                    else if (key == "rule_custom_poise") config.Rules.CustomPoise = value;
                    else if (key == "play_with") config.PlayWith = (value == InputConfig.Keyboard || value == InputConfig.Controller) ? value : "";
                    else if (key == "keep_server") config.KeepServer = (value == "1");
                    else if (key == "stop_server_with_game") config.StopServerWithGame = (value != "0");
                }
            }
            catch (Exception error)
            {
                Log.Line("Configuração não lida (" + error.Message + "); usando os valores padrão.");
            }
            // The account the co-op signs in with: what was kept here, else the one already made in the
            // bbport launcher's Seamless co-op page (the one the player signs in with there), else a fresh
            // one. The server makes an account at its first sign-in (its own MakeAccountsAtSignIn), so any
            // valid id works - but reusing the launcher's keeps the player on the account they know.
            if (!CoopSchema.ValidAccount(config.Account))
            {
                string known = BbportLauncher.Account();
                config.Account = CoopSchema.ValidAccount(known) ? known : NewAccount();
            }
            if (config.Password.Length == 0)
            {
                string known = BbportLauncher.Password();
                config.Password = known.Length > 0 ? known : NewPassword();
            }
            return config;
        }

        /// <summary>
        /// Fills the account and password from the bbport launcher's own settings, the ones made on its
        /// Seamless co-op page: the account the player already signs in with there. False when it has none.
        /// </summary>
        public bool TakeFromBbportLauncher()
        {
            string account = BbportLauncher.Account();
            string password = BbportLauncher.Password();
            if (account.Length == 0 && password.Length == 0) return false;
            if (account.Length > 0) Account = account;
            if (password.Length > 0) Password = password;
            return true;
        }

        /// <summary>Takes the host rules chosen on the launcher's Seamless co-op page as this window's own.
        /// False when the launcher has no settings to read.</summary>
        public bool TakeRulesFromBbportLauncher()
        {
            BbportLauncher.Invalidate();
            if (!File.Exists(BbportLauncher.SettingsPath)) return false;
            Rules = BbportLauncher.Rules();
            return true;
        }

        public void Save()
        {
            try
            {
                Directory.CreateDirectory(Folder);
                List<string> lines = new List<string>();
                lines.Add("# BBRadmin: as respostas do app. Não é do bbport; pode apagar este arquivo.");
                lines.Add("bbport_dir=" + BbportDir);
                lines.Add("role=" + Role);
                lines.Add("account=" + Account);
                lines.Add("password=" + Protect(Password));
                lines.Add("remote_ip=" + RemoteIp);
                lines.Add("rule_rot=" + (Rules.Rot ? "1" : "0"));
                lines.Add("rule_travel=" + (Rules.Travel ? "1" : "0"));
                lines.Add("rule_vote=" + (Rules.Vote ? "1" : "0"));
                lines.Add("rule_revive=" + (Rules.Revive ? "1" : "0"));
                lines.Add("rule_enemies=" + Rules.Enemies);
                lines.Add("rule_tempo=" + Rules.Tempo);
                lines.Add("rule_custom_hp=" + Rules.CustomHp);
                lines.Add("rule_custom_damage=" + Rules.CustomDamage);
                lines.Add("rule_custom_poise=" + Rules.CustomPoise);
                lines.Add("play_with=" + PlayWith);
                lines.Add("keep_server=" + (KeepServer ? "1" : "0"));
                lines.Add("stop_server_with_game=" + (StopServerWithGame ? "1" : "0"));
                File.WriteAllLines(FilePath, lines.ToArray(), new UTF8Encoding(false));
            }
            catch (Exception error)
            {
                Log.Line("Configuração não gravada (" + error.Message + ").");
            }
        }

        /// <summary>A player id shadnet accepts, one per PC (the server makes the account at its first link).</summary>
        public static string NewAccount()
        {
            string name = Environment.UserName;
            StringBuilder clean = new StringBuilder();
            foreach (char c in name)
            {
                if (char.IsLetterOrDigit(c) || c == '_' || c == '-') clean.Append(c);
            }
            string stem = clean.ToString();
            if (stem.Length < 3) stem = "hunter";
            if (stem.Length > 12) stem = stem.Substring(0, 12);
            return stem + "-" + new Random().Next(100, 999).ToString(CultureInfo.InvariantCulture);
        }

        public static string NewPassword()
        {
            const string alphabet = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
            byte[] bytes = new byte[16];
            using (RandomNumberGenerator rng = RandomNumberGenerator.Create()) rng.GetBytes(bytes);
            StringBuilder text = new StringBuilder();
            foreach (byte b in bytes) text.Append(alphabet[b % alphabet.Length]);
            return text.ToString();
        }

        private static string Protect(string value)
        {
            if (string.IsNullOrEmpty(value)) return "";
            try
            {
                byte[] protectedBytes = ProtectedData.Protect(Encoding.UTF8.GetBytes(value), null, DataProtectionScope.CurrentUser);
                return "dpapi:" + Convert.ToBase64String(protectedBytes);
            }
            catch (Exception)
            {
                return "";  // not persisted rather than kept in the open
            }
        }

        private static string Unprotect(string value)
        {
            if (string.IsNullOrEmpty(value) || !value.StartsWith("dpapi:")) return "";
            try
            {
                byte[] bytes = Convert.FromBase64String(value.Substring(6));
                return Encoding.UTF8.GetString(ProtectedData.Unprotect(bytes, null, DataProtectionScope.CurrentUser));
            }
            catch (Exception)
            {
                return "";
            }
        }
    }
}
