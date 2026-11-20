using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Text;

namespace BBRadmin.Core
{
    internal sealed class FirewallRule
    {
        public string Name;
        public string Protocol;
        public int Port;
    }

    /// <summary>
    /// Windows Firewall for the host: the co-op server's ports, open to VPN addresses and the local
    /// network only (the same idea as BBSC's own rules: Radmin 26/8, Hamachi 25/8, Tailscale 100.64/10,
    /// and this PC's own subnet — never the internet).
    ///
    /// netsh is used because it is on every Windows: no PowerShell, no admin needed to *read* a rule.
    /// Installing needs admin, so the app restarts itself with --install-firewall (UAC once).
    /// </summary>
    internal static class Firewall
    {
        public const string RemoteRanges = "26.0.0.0/8,25.0.0.0/8,100.64.0.0/10,LocalSubnet";

        public static readonly FirewallRule[] Rules =
        {
            Make("BBRadmin link 31314", "UDP", CoopSchema.LinkPort),
            Make("BBRadmin webapi 31315", "TCP", CoopSchema.WebApiPort),
            Make("BBRadmin game server 20443", "TCP", CoopSchema.BackendPort),
        };

        private static FirewallRule Make(string name, string protocol, int port)
        {
            FirewallRule rule = new FirewallRule();
            rule.Name = name;
            rule.Protocol = protocol;
            rule.Port = port;
            return rule;
        }

        public static bool Installed(FirewallRule rule)
        {
            string output = Run("advfirewall firewall show rule name=\"" + rule.Name + "\"");
            // The rule's own line has the port and the ranges; a localized "no rules" answer has neither.
            return output.Contains(rule.Port.ToString(CultureInfo.InvariantCulture)) &&
                   output.Contains("26.0.0.0/8");
        }

        public static List<string> Missing()
        {
            List<string> missing = new List<string>();
            foreach (FirewallRule rule in Rules)
            {
                if (!Installed(rule)) missing.Add(rule.Name);
            }
            return missing;
        }

        /// <summary>One elevated call that replaces our three rules and prints what happened.</summary>
        public static bool InstallAll(out string report)
        {
            StringBuilder text = new StringBuilder();
            bool ok = true;
            foreach (FirewallRule rule in Rules)
            {
                string delete = "advfirewall firewall delete rule name=\"" + rule.Name + "\"";
                Run(delete);
                string add = "advfirewall firewall add rule name=\"" + rule.Name + "\" dir=in action=allow protocol=" +
                             rule.Protocol + " localport=" + rule.Port.ToString(CultureInfo.InvariantCulture) +
                             " remoteip=" + RemoteRanges;
                string output = Run(add);
                bool made = Installed(rule);
                if (!made) ok = false;
                text.AppendLine((made ? "ok   " : "FALHA ") + rule.Name + " (" + rule.Protocol + " " +
                                rule.Port.ToString(CultureInfo.InvariantCulture) + ")");
                if (!made && output.Length > 0) text.AppendLine("      " + output.Replace("\r\n", " ").Trim());
            }
            report = text.ToString();
            return ok;
        }

        /// <summary>The lines a player can run by hand in an admin console, when UAC is not an option.</summary>
        public static string ManualCommands()
        {
            StringBuilder text = new StringBuilder();
            foreach (FirewallRule rule in Rules)
            {
                text.AppendLine("netsh advfirewall firewall add rule name=\"" + rule.Name + "\" dir=in action=allow protocol=" +
                                rule.Protocol + " localport=" + rule.Port.ToString(CultureInfo.InvariantCulture) +
                                " remoteip=" + RemoteRanges);
            }
            return text.ToString();
        }

        private static string Run(string arguments)
        {
            try
            {
                ProcessStartInfo start = new ProcessStartInfo("netsh.exe", arguments);
                start.UseShellExecute = false;
                start.CreateNoWindow = true;
                start.RedirectStandardOutput = true;
                start.RedirectStandardError = true;
                using (Process process = Process.Start(start))
                {
                    string output = process.StandardOutput.ReadToEnd() + process.StandardError.ReadToEnd();
                    process.WaitForExit(15000);
                    return output;
                }
            }
            catch (Exception error)
            {
                Log.Line("netsh falhou (" + error.Message + ").");
                return "";
            }
        }
    }
}
