using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>
    /// <bbport>\seamless\user\host_overrides.custom.json — the game's own server sent to the host's
    /// co-op backend (BBSC 1.0.0: bbport_yharnam.write_host_overrides; the port's own run.py reads this
    /// very file through SHADPS4_HTTP_HOST_OVERRIDES_JSON).
    ///
    /// One key only. The file is written with a backup beside it, and never invented from nothing: a
    /// player's own file with other addresses keeps them (BBRadmin replaces the file only when its
    /// content is its own).
    /// </summary>
    internal static class Overrides
    {
        private const string BackupSuffix = ".bbradmin-bak";
        private const string StampPrefix = "bbradmin-";

        public static string PathIn(BbportInfo port)
        {
            return Path.Combine(port.Root, CoopSchema.OverridesFile);
        }

        public static string Text(string hostIp)
        {
            StringBuilder json = new StringBuilder();
            json.Append("{\n");
            json.Append("  \"").Append(CoopSchema.GameServer).Append("\": \"http://").Append(hostIp)
                .Append(":").Append(CoopSchema.BackendPort.ToString(CultureInfo.InvariantCulture)).Append("\"\n");
            json.Append("}\n");
            return json.ToString();
        }

        /// <summary>Writes the redirect; returns a line for the player/log. Never throws.</summary>
        public static string Write(BbportInfo port, string hostIp, out string error)
        {
            error = "";
            string path = PathIn(port);
            string wanted = Text(hostIp);
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                string current = File.Exists(path) ? File.ReadAllText(path, Encoding.UTF8) : "";
                if (Normalize(current) == Normalize(wanted))
                {
                    return "Overrides já apontavam para " + hostIp + " (nada mudou).";
                }
                if (File.Exists(path)) Backup(path);
                File.WriteAllText(path, wanted, new UTF8Encoding(false));
                return "Overrides gravados: " + CoopSchema.OverridesFile + " → http://" + hostIp + ":" + CoopSchema.BackendPort;
            }
            catch (Exception exception)
            {
                error = "Não escrevi " + CoopSchema.OverridesFile + ": " + exception.Message;
                return error;
            }
        }

        /// <summary>True when the file is the one BBRadmin writes (so replacing it loses nothing).</summary>
        public static bool IsOurs(string text)
        {
            if (string.IsNullOrEmpty(text)) return false;
            return text.Contains(CoopSchema.GameServer) && text.Contains("http://") && !text.Contains("summon_messenger");
        }

        public static string Current(BbportInfo port)
        {
            try
            {
                string path = PathIn(port);
                return File.Exists(path) ? File.ReadAllText(path, Encoding.UTF8) : "";
            }
            catch (Exception)
            {
                return "";
            }
        }

        /// <summary>Puts back the newest backup (and says so), or removes the file when there was none.</summary>
        public static string Restore(BbportInfo port, out string error)
        {
            error = "";
            string path = PathIn(port);
            try
            {
                if (!Directory.Exists(Path.GetDirectoryName(path))) return "Nada a restaurar: a pasta do co-op não existe.";
                List<string> backups = new List<string>(Directory.GetFiles(Path.GetDirectoryName(path),
                    Path.GetFileName(path) + BackupSuffix + "*"));
                if (backups.Count == 0)
                {
                    if (File.Exists(path))
                    {
                        File.Delete(path);
                        return "Overrides removidos (não havia backup: o bbport volta ao arquivo próprio dele).";
                    }
                    return "Nada a restaurar.";
                }
                backups.Sort(StringComparer.Ordinal);
                string newest = backups[backups.Count - 1];
                File.Copy(newest, path, true);
                File.Delete(newest);
                return "Overrides restaurados do backup " + Path.GetFileName(newest) + ".";
            }
            catch (Exception exception)
            {
                error = "Não restaurei os overrides: " + exception.Message;
                return error;
            }
        }

        private static void Backup(string path)
        {
            string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss", CultureInfo.InvariantCulture);
            string target = path + BackupSuffix + "-" + StampPrefix + stamp;
            try
            {
                File.Copy(path, target, true);
            }
            catch (Exception error)
            {
                Log.Line("Backup dos overrides não feito (" + error.Message + ").");
            }
        }

        private static string Normalize(string text)
        {
            return (text ?? "").Replace("\r\n", "\n").Trim();
        }
    }
}
