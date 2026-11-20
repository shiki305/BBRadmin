using System;
using System.IO;
using System.Text;
using System.Windows.Forms;
using BBRadmin.Core;

namespace BBRadmin
{
    /// <summary>
    /// BBRadmin: the co-op start for two players on one Radmin VPN, exactly as the port's own launcher
    /// does it. Two ways in stay deliberately separate:
    ///
    ///   BBRadmin.exe                  the window (this app is the app people use)
    ///   BBRadmin.exe --dry-run        what a start would do, written down, nothing done
    ///   BBRadmin.exe --install-firewall   the elevated half: one netsh call for the three rules
    ///
    /// The game's own launcher (Bloodborne.exe with no argument) is never started here as the play path:
    /// it is untouched and stays reachable from the "Avançado" tab.
    /// </summary>
    internal static class Program
    {
        public static string FirewallReportPath
        {
            get { return Path.Combine(AppConfig.Folder, "firewall-report.txt"); }
        }

        [STAThread]
        private static int Main(string[] args)
        {
            bool dryRun = false;
            bool installFirewall = false;
            foreach (string raw in args)
            {
                string name = raw.TrimStart('-', '/').ToLowerInvariant();
                if (name == "dry-run" || name == "dryrun") dryRun = true;
                else if (name == "install-firewall") installFirewall = true;
            }

            if (installFirewall) return InstallFirewall();

            AppConfig config = AppConfig.Load();

            if (dryRun)
            {
                // No window: the report goes to the console this was started from (and to a file).
                return DryRun.Run(config);
            }

            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.Run(new MainForm(config));
            return 0;
        }

        /// <summary>
        /// The elevated half of the firewall step: a second copy of this exe, started by the window with
        /// verb "runas" (one UAC prompt), adds the three rules and writes what happened to a file the
        /// window then reads. Nothing else is elevated.
        /// </summary>
        private static int InstallFirewall()
        {
            string report;
            bool ok;
            try
            {
                ok = Firewall.InstallAll(out report);
            }
            catch (Exception error)
            {
                ok = false;
                report = "ERRO: " + error.Message;
            }

            try
            {
                Directory.CreateDirectory(AppConfig.Folder);
                File.WriteAllText(FirewallReportPath, report, new UTF8Encoding(false));
            }
            catch (Exception error)
            {
                Log.Line("Relatório do firewall não gravado (" + error.Message + ").");
            }

            return ok ? 0 : 1;
        }
    }
}
