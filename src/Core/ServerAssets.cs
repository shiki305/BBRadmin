using System;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>
    /// The co-op server BBRadmin runs on the host: BBSC: Yharnam Together 1.0.0's own Windows server
    /// (shadnet.exe and its Qt libraries), the very release bbport's Seamless co-op is built from
    /// (BBSC-Yharnam-Together-1.0.0-BBport.zip, commit cdb58df0, co-op protocol 9).
    ///
    /// Why BBRadmin brings its own: bbport 1.9.8.6 ships the co-op *client* only — its seamless folder
    /// has no server, and its launcher can only join one. A server of another release would be refused
    /// ("the server refused this game": the fingerprint says version=1.5.0, this game says 1.0.0), which
    /// is why the RadminBell that ships Yharnam Together 1.5.0's server does not work with bbport here.
    ///
    /// The files travel inside BBRadmin.exe (one resource, a zip) and are unpacked once into
    /// %LOCALAPPDATA%\BBRadmin. Nothing of the player's bbport folder is written from here.
    /// </summary>
    internal static class ServerAssets
    {
        private const string ResourceName = "BBRadmin.server.zip";
        private const string LicenseName = "LICENSE-BBSC.txt";

        public static string Folder
        {
            get { return Path.Combine(AppConfig.Folder, CoopSchema.ServerFolder); }
        }

        public static string ExePath
        {
            get { return Path.Combine(Folder, CoopSchema.ServerExe); }
        }

        public static string ConfigPath
        {
            get { return Path.Combine(Folder, CoopSchema.ServerConfig); }
        }

        public static string LogFolder
        {
            get { return Path.Combine(AppConfig.Folder, CoopSchema.ServerLogFolder); }
        }

        public static string LogPath
        {
            get { return Path.Combine(LogFolder, CoopSchema.ServerLog); }
        }

        public static string PrevLogPath
        {
            get { return Path.Combine(LogFolder, CoopSchema.ServerPrevLog); }
        }

        public static string BackendFolder
        {
            get { return Path.Combine(AppConfig.Folder, CoopSchema.BackendFolder); }
        }

        public static string StampPath
        {
            get { return Path.Combine(AppConfig.Folder, CoopSchema.ServerStamp); }
        }

        /// <summary>True when this release's server is unpacked and complete.</summary>
        public static bool Installed()
        {
            if (!File.Exists(ExePath)) return false;
            foreach (string name in CoopSchema.ServerFiles)
            {
                if (!File.Exists(Path.Combine(Folder, name))) return false;
            }
            return Stamp() == CoopSchema.ReleaseVersion;
        }

        /// <summary>One line for the window and the dry-run.</summary>
        public static string State()
        {
            if (!File.Exists(ExePath)) return "servidor do co-op ainda não instalado (ele entra sozinho ao hospedar)";
            if (Stamp() != CoopSchema.ReleaseVersion) return "servidor do co-op instalado em versão antiga (hospede de novo para atualizar)";
            return "servidor do co-op 1.0.0 pronto";
        }

        private static string Stamp()
        {
            try
            {
                return File.Exists(StampPath) ? File.ReadAllText(StampPath, Encoding.UTF8).Trim() : "";
            }
            catch (Exception)
            {
                return "";
            }
        }

        /// <summary>Unpacks the server beside BBRadmin's own files when it is not there yet. Never throws.</summary>
        public static string Ensure(out string error)
        {
            error = "";
            if (Installed()) return "Servidor do co-op já instalado.";

            string zip = Path.Combine(Path.GetTempPath(), "bbradmin-server-" + Guid.NewGuid().ToString("N") + ".zip");
            string staging = Path.Combine(Path.GetTempPath(), "bbradmin-server-" + Guid.NewGuid().ToString("N"));
            try
            {
                using (Stream resource = Assembly.GetExecutingAssembly().GetManifestResourceStream(ResourceName))
                {
                    if (resource == null)
                    {
                        error = "Este BBRadmin.exe foi compilado sem o servidor do co-op dentro.";
                        return error;
                    }
                    using (FileStream file = File.Create(zip)) resource.CopyTo(file);
                }

                Directory.CreateDirectory(staging);
                ZipFile.ExtractToDirectory(zip, staging);

                string unpacked = Path.Combine(staging, CoopSchema.ServerFolder);
                if (!File.Exists(Path.Combine(unpacked, CoopSchema.ServerExe)))
                {
                    error = "O pacote do servidor não trazia o " + CoopSchema.ServerExe + ".";
                    return error;
                }

                // Copied over, not replaced: the server keeps its own state in this folder (its database
                // under db\, and link_key.bin, the key the clients pin on first use), and an update of
                // BBRadmin's server assets must not throw that away.
                Directory.CreateDirectory(Folder);
                CopyOver(unpacked, Folder);

                // The server is GPL-2.0; its license and its origin travel beside it.
                string license = Path.Combine(staging, LicenseName);
                if (File.Exists(license)) File.Copy(license, Path.Combine(AppConfig.Folder, LicenseName), true);

                // shadnet.cfg's own BackendFolder=../backend: the folder shadnet reads its notices from.
                if (!Directory.Exists(BackendFolder)) Directory.CreateDirectory(BackendFolder);
                string notice = Path.Combine(BackendFolder, CoopSchema.BackendNotice);
                if (!File.Exists(notice)) File.WriteAllText(notice, "", new UTF8Encoding(false));

                File.WriteAllText(StampPath, CoopSchema.ReleaseVersion + Environment.NewLine, new UTF8Encoding(false));
                Log.Line("Servidor do co-op 1.0.0 instalado em " + Folder + ".");
                return "Servidor do co-op 1.0.0 instalado.";
            }
            catch (Exception exception)
            {
                error = "Não instalei o servidor do co-op: " + exception.Message;
                return error;
            }
            finally
            {
                try { if (File.Exists(zip)) File.Delete(zip); } catch (Exception) { }
                try { if (Directory.Exists(staging)) Directory.Delete(staging, true); } catch (Exception) { }
            }
        }

        /// <summary>Copies a folder's files over the target, creating the subfolders it needs (never deletes).</summary>
        private static void CopyOver(string from, string to)
        {
            foreach (string file in Directory.GetFiles(from))
            {
                File.Copy(file, Path.Combine(to, Path.GetFileName(file)), true);
            }
            foreach (string folder in Directory.GetDirectories(from))
            {
                string name = Path.GetFileName(folder);
                Directory.CreateDirectory(Path.Combine(to, name));
                CopyOver(folder, Path.Combine(to, name));
            }
        }
    }
}
