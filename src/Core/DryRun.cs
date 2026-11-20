using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>
    /// --dry-run: what a start would do, written down and nothing done. It is the test to run after every
    /// bbport update: the folder, the address, the overrides and the environment are shown; the game is
    /// not started and no file of the player's is written.
    /// </summary>
    internal static class DryRun
    {
        [DllImport("kernel32.dll")]
        private static extern bool AttachConsole(int processId);

        private const int AttachParentProcess = -1;

        public static string FilePath
        {
            get { return Path.Combine(AppConfig.Folder, "dry-run.txt"); }
        }

        public static int Run(AppConfig config)
        {
            StringBuilder report = new StringBuilder();
            report.AppendLine("BBRadmin · dry-run · " + DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
            report.AppendLine();

            string root = config.BbportDir;
            if (string.IsNullOrEmpty(root) || !Directory.Exists(root))
            {
                string guess = Bbport.GuessFolder();
                report.AppendLine("Pasta do bbport: não escolhida" + (guess.Length > 0 ? " (encontrei esta: " + guess + ")" : ""));
                if (guess.Length > 0) root = guess;
            }
            BbportInfo port = Bbport.Inspect(root);
            report.AppendLine("Pasta do bbport: " + (port.Root.Length > 0 ? port.Root : "(nenhuma)"));
            report.AppendLine("Versão: " + (port.Version.Length > 0 ? port.Version : "(não lida)"));
            report.AppendLine("Pronto para o co-op: " + (port.Ready ? "sim" : "NÃO"));
            foreach (string missing in port.Missing) report.AppendLine("  FALTA: " + missing);
            foreach (string note in port.Notes) report.AppendLine("  aviso: " + note);
            report.AppendLine();

            List<NetworkChoice> networks = Radmin.Detect();
            report.AppendLine("Endereços deste PC:");
            if (networks.Count == 0) report.AppendLine("  nenhum (o Radmin VPN está ligado?)");
            foreach (NetworkChoice choice in networks) report.AppendLine("  " + (Radmin.IsVpn(choice.Kind) ? "[VPN] " : "      ") + choice.Describe());
            NetworkChoice pick = Radmin.Pick(networks);
            string hostIp = (pick != null ? pick.Ip : "");
            report.AppendLine("Endereço que um start usaria: " + (hostIp.Length > 0 ? hostIp : "(nenhum)"));
            report.AppendLine();

            if (!port.Ready || hostIp.Length == 0)
            {
                report.AppendLine("Sem pasta do bbport ou sem endereço: um start não seria possível.");
                report.AppendLine("O arquivo do bbport e o servidor NÃO foram tocados.");
                return Finish(report);
            }

            report.AppendLine("Conta: " + config.Account + "  (senha: " + Mask(config.Password) + ")");
            report.AppendLine("Regras do host: " + config.Rules.Describe());
            report.AppendLine("Controles (" + CoopSchema.InputFile + "): " + InputConfig.Describe(port) +
                              (config.PlayWith.Length > 0 ? "  ·  um start deixaria: " + config.PlayWith : "  ·  um start não mexeria"));
            report.AppendLine();

            PlayPlan plan = GameEnvironment.Build(port, hostIp, config.Account, config.Password, config.Rules);
            report.AppendLine(plan.Report(false));

            report.AppendLine();
            report.AppendLine("--- o que um HOSPEDAR faria ---");
            report.AppendLine("  iniciar: " + ServerAssets.ExePath + " --watchdog");
            report.AppendLine("  pasta:   " + ServerAssets.Folder + "  (" + ServerAssets.State() + ")");
            report.AppendLine("  log:     " + ServerAssets.LogPath);
            report.AppendLine("  firewall: " + FirewallState());
            report.AppendLine();

            report.AppendLine("--- comandos de firewall (para um console de administrador) ---");
            report.Append(Firewall.ManualCommands());
            report.AppendLine();

            string current = Overrides.Current(port);
            report.AppendLine("--- " + CoopSchema.OverridesFile + " agora ---");
            report.AppendLine(current.Length > 0 ? current.TrimEnd() : "(não existe)");
            report.AppendLine();
            report.AppendLine("Um start real gravaria o arquivo acima (com backup ao lado) e iniciaria:");
            report.AppendLine("  " + Path.Combine(port.Root, CoopSchema.GameExe) + " --run");
            report.AppendLine();
            report.AppendLine("Nada foi iniciado nem gravado por este dry-run.");
            return Finish(report);
        }

        private static string FirewallState()
        {
            List<string> missing = Firewall.Missing();
            if (missing.Count == 0) return "as três regras do BBRadmin estão instaladas";
            return "faltam: " + string.Join(", ", missing.ToArray());
        }

        private static string Mask(string password)
        {
            return string.IsNullOrEmpty(password) ? "(vazia)" : new string('•', Math.Min(12, password.Length));
        }

        private static int Finish(StringBuilder report)
        {
            try
            {
                Directory.CreateDirectory(AppConfig.Folder);
                File.WriteAllText(FilePath, report.ToString(), new UTF8Encoding(false));
            }
            catch (Exception error)
            {
                Log.Line("dry-run não gravado (" + error.Message + ").");
            }
            try
            {
                if (AttachConsole(AttachParentProcess))
                {
                    Console.Out.Write(report.ToString());
                    Console.Out.Write("Relatório também em: " + FilePath + Environment.NewLine);
                }
            }
            catch (Exception) { }
            if (Environment.GetEnvironmentVariable("BBRADMIN_VERBOSE") == "1")
            {
                try { System.Windows.Forms.MessageBox.Show(report.ToString(), "BBRadmin — dry-run"); }
                catch (Exception) { }
            }
            return 0;
        }
    }
}
