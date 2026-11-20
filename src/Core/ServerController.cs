using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text;
using System.Threading;

namespace BBRadmin.Core
{
    /// <summary>
    /// The co-op server on the host's PC: shadnet.exe — accounts, matchmaking, the party boards and the
    /// game's own backend port. It is the server of BBSC: Yharnam Together 1.0.0, the release bbport's
    /// Seamless co-op is built from (see ServerAssets), started the way Yharnam Together starts its own
    /// server (launcher/bbport_coop.start_servers):
    ///
    ///   shadnet.exe --watchdog, cwd = the server folder,
    ///   SHADNET_BACKEND_ADDRESS = this PC's address,
    ///   BB_RELEASE_VERSION = 1.0.0 (else the server would announce "dev" and refuse the game),
    ///   QT_FORCE_STDERR_LOGGING=1,
    ///   output written into %LOCALAPPDATA%\BBRadmin\log\shadnet.txt (the run before it becomes
    ///   shadnet.prev.txt), and the server does NOT belong to this window: closing BBRadmin leaves a hunt
    ///   in progress alone (unless the player asked otherwise).
    ///
    /// YT starts it with DETACHED_PROCESS, which .NET Framework cannot ask for; a one-line cmd.exe wrapper
    /// does the same thing here — the server is cmd's child, has no console of its own, and its output goes
    /// straight to the log file (a pipe owned by BBRadmin would have ended with it).
    /// </summary>
    internal sealed class ServerController
    {
        private readonly object _gate = new object();
        private Process _process;
        private bool _startedByUs;
        private string _logPath = "";

        public bool StartedByUs
        {
            get { lock (_gate) return _startedByUs; }
        }

        /// <summary>The server answers on its own listener (31313, the port the state is read from).</summary>
        public bool Running()
        {
            return Health.Tcp("127.0.0.1", CoopSchema.RpcPort, 400);
        }

        /// <summary>shadnet runs but the game's own port does not answer: YT's "Running, but 20443..." case.</summary>
        public bool BackendAnswers()
        {
            return Health.Tcp("127.0.0.1", CoopSchema.BackendPort, 400);
        }

        public string State()
        {
            if (!Running()) return "parado";
            return BackendAnswers() ? "em execução" : "em execução, mas o backend (20443) não responde";
        }

        public string Start(string hostIp, out string error)
        {
            error = "";
            if (Running()) return "O servidor já estava em execução.";

            // The 1.0.0 server travels inside BBRadmin.exe; the first host unpacks it here. Nothing of
            // the player's bbport folder is written.
            string prepared = ServerAssets.Ensure(out error);
            if (error.Length > 0) return prepared;

            string serverDir = ServerAssets.Folder;
            string exe = ServerAssets.ExePath;
            if (!File.Exists(exe))
            {
                error = "shadnet.exe não está em " + serverDir + ".";
                return error;
            }

            string logPath = ServerAssets.LogPath;
            try
            {
                Directory.CreateDirectory(ServerAssets.LogFolder);
                if (File.Exists(logPath))
                {
                    // The log of the run before this one, as YT keeps it (shadnet.txt is rewritten).
                    File.Copy(logPath, ServerAssets.PrevLogPath, true);
                }
                // shadnet.cfg's own BackendFolder=../backend: the folder it reads its notices from.
                Directory.CreateDirectory(ServerAssets.BackendFolder);
            }
            catch (Exception exception)
            {
                Log.Line("Log do servidor não preparado (" + exception.Message + ").");
            }

            // cmd.exe needs the two paths quoted; the redirection belongs to the wrapper, so the server
            // keeps writing its log whether or not this window is still open.
            string line = "/c \"\"" + exe + "\" --watchdog > \"" + logPath + "\" 2>&1\"";
            ProcessStartInfo start = new ProcessStartInfo("cmd.exe");
            start.Arguments = line;
            start.WorkingDirectory = serverDir;
            start.UseShellExecute = false;
            start.CreateNoWindow = true;
            start.EnvironmentVariables[CoopSchema.EnvBackendAddress] = hostIp;
            start.EnvironmentVariables[CoopSchema.EnvReleaseVersion] = CoopSchema.ReleaseVersion;
            start.EnvironmentVariables[CoopSchema.EnvQtStderrLogging] = "1";
            start.EnvironmentVariables[CoopSchema.EnvQtLoggingRules] = "*.debug=false";

            try
            {
                _process = Process.Start(start);
                _logPath = logPath;
                lock (_gate) _startedByUs = true;
                Log.Line("Servidor iniciado: " + exe + " --watchdog (backend em " + hostIp + ", log " + logPath + ").");
                return "Servidor iniciado (shadnet).";
            }
            catch (Exception exception)
            {
                error = "Não iniciei o servidor: " + exception.Message;
                return error;
            }
        }

        /// <summary>Waits for the server and its backend, up to the given time. Returns a reading line.</summary>
        public string WaitUntilReady(string hostIp, int seconds)
        {
            DateTime deadline = DateTime.Now.AddSeconds(seconds);
            while (DateTime.Now < deadline)
            {
                if (Running() && BackendAnswers()) break;
                Thread.Sleep(300);
            }
            if (!Running()) return "O servidor não respondeu na porta 31313. Veja o log do servidor.";
            if (!BackendAnswers()) return "O servidor responde, mas o backend (20443) não. Pare e inicie o servidor de novo.";
            return "Servidor pronto · " + Health.Describe(hostIp, 900);
        }

        /// <summary>Stops the server: our own wrapper's process tree, or a shadnet left from another start.</summary>
        public string Stop()
        {
            Process process;
            lock (_gate) process = _process;

            if (process != null)
            {
                try
                {
                    if (!process.HasExited) TaskKillPid(process.Id);
                }
                catch (Exception exception)
                {
                    Log.Line("Parar o servidor pelo pid falhou (" + exception.Message + ").");
                }
                lock (_gate) _process = null;
            }

            // The watchdog restarts a crashed server, so kill by image too, and check again: YT's own way.
            for (int attempt = 0; attempt < 8 && Running(); attempt++)
            {
                TaskKillImage("shadnet.exe");
                Thread.Sleep(600);
            }

            lock (_gate) _startedByUs = false;
            bool stopped = !Running();
            Log.Line(stopped ? "Servidor parado." : "O servidor continua respondendo na 31313.");
            return stopped ? "Servidor parado." : "O servidor ainda responde na porta 31313 (feche-o pelo Gerenciador de Tarefas).";
        }

        /// <summary>The server's own log file, as the window's log view shows it.</summary>
        public string Recent(int lines)
        {
            string path = _logPath;
            if (path.Length == 0) path = ServerAssets.LogPath;
            return Tail(path, lines);
        }

        private static string Tail(string path, int lines)
        {
            try
            {
                if (path.Length == 0 || !File.Exists(path)) return "";
                // The running server (through cmd's redirection) holds this file open: read it while it
                // is being written.
                string[] all;
                using (FileStream stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
                using (StreamReader reader = new StreamReader(stream, Encoding.UTF8))
                {
                    all = reader.ReadToEnd().Replace("\r\n", "\n").Split('\n');
                }
                int from = Math.Max(0, all.Length - lines);
                StringBuilder text = new StringBuilder();
                for (int i = from; i < all.Length; i++)
                {
                    if (i == all.Length - 1 && all[i].Length == 0) continue;  // the trailing newline
                    text.AppendLine(all[i]);
                }
                return text.ToString();
            }
            catch (Exception)
            {
                return "";
            }
        }

        private static void TaskKillPid(int pid)
        {
            Run("taskkill.exe", "/PID " + pid.ToString(CultureInfo.InvariantCulture) + " /T /F");
        }

        private static void TaskKillImage(string image)
        {
            Run("taskkill.exe", "/IM " + image + " /T /F");
        }

        private static void Run(string file, string arguments)
        {
            try
            {
                ProcessStartInfo start = new ProcessStartInfo(file, arguments);
                start.UseShellExecute = false;
                start.CreateNoWindow = true;
                start.RedirectStandardOutput = true;
                start.RedirectStandardError = true;
                using (Process process = Process.Start(start))
                {
                    process.StandardOutput.ReadToEnd();
                    process.StandardError.ReadToEnd();
                    process.WaitForExit(15000);
                }
            }
            catch (Exception error)
            {
                Log.Line(file + " " + arguments + " falhou (" + error.Message + ").");
            }
        }
    }
}
