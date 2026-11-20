using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

namespace BBRadmin.Core
{
    /// <summary>What was found in a bbport folder.</summary>
    internal sealed class BbportInfo
    {
        public string Root = "";
        public string Version = "";
        public string CoopRelease = "";                     // BBSC release bbport's seamless folder is
        public List<string> Missing = new List<string>();   // without these the co-op start cannot work
        public List<string> Notes = new List<string>();     // still playable, but something is not there

        public bool Ready
        {
            get { return Root.Length > 0 && Missing.Count == 0; }
        }

        public string Describe()
        {
            if (Root.Length == 0) return "pasta do bbport ainda não escolhida";
            if (!Ready) return "faltando: " + string.Join(", ", Missing.ToArray());
            return (Version.Length > 0 ? "bbport " + Version : "bbport") +
                   (CoopRelease.Length > 0 ? " · co-op " + CoopRelease : "") + " · " + Root;
        }
    }

    /// <summary>Finds and checks the bbport install (github.com/Supermedo/bloodborne_pc).</summary>
    internal static class Bbport
    {
        public static BbportInfo Inspect(string root)
        {
            BbportInfo info = new BbportInfo();
            if (string.IsNullOrEmpty(root)) return info;
            info.Root = root;
            if (!Directory.Exists(root))
            {
                info.Missing.Add("a pasta não existe");
                return info;
            }

            // What a co-op start needs of bbport itself: the port, its seamless engine (this port's own
            // engine, built with BBSC's co-op) and the co-op saves folder. The co-op server is NOT here:
            // bbport 1.9.8.6 ships the client half only, so BBRadmin brings the 1.0.0 server itself
            // (ServerAssets); the port's launcher can only *join* one.
            Require(info, CoopSchema.GameExe, "Bloodborne.exe");
            Require(info, CoopSchema.EngineSeamless, @"bin\bb-probe-seamless.exe");
            Require(info, CoopSchema.CoopUserDir, @"seamless\user");

            // Prepared by the port's own Seamless co-op page (BBSC's view, events and mods layer). The
            // game needs them; the moment they are missing is told to the player, not hidden.
            Note(info, CoopSchema.CoopGameView, @"seamless\coop-game\CUSA03173 (a view of the game: abra o Seamless co-op uma vez no launcher do bbport)");
            Note(info, CoopSchema.CoopModsView, @"seamless\mods-view (o co-op vem dela: abra o Seamless co-op uma vez no launcher do bbport)");

            info.Version = ReadVersion(root);
            info.CoopRelease = ReadCoopRelease(root);
            if (info.CoopRelease.Length > 0 && info.CoopRelease != CoopSchema.ReleaseVersion)
            {
                info.Notes.Add("o co-op do bbport é a versão " + info.CoopRelease + ", e o servidor do " +
                               "BBRadmin é a " + CoopSchema.ReleaseVersion + ": o servidor recusa um jogo de " +
                               "outra versão, então hospedar só funciona com as duas iguais");
            }
            return info;
        }

        /// <summary>The version the launcher's own window shows: the exe's file version, else the port's
        /// own manifest (bbport-files.json: "version": "1.9.8.6"), which every recent port carries.</summary>
        private static string ReadVersion(string root)
        {
            string[] candidates = { CoopSchema.GameExe, CoopSchema.EngineSeamless };
            foreach (string relative in candidates)
            {
                try
                {
                    string path = Path.Combine(root, relative);
                    if (!File.Exists(path)) continue;
                    FileVersionInfo info = FileVersionInfo.GetVersionInfo(path);
                    string text = (info.FileVersion ?? "").Trim();
                    if (text.Length == 0) text = (info.ProductVersion ?? "").Trim();
                    Match match = Regex.Match(text, @"\d+\.\d+(\.\d+){0,2}");
                    if (match.Success) return match.Value;
                }
                catch (Exception)
                {
                    // no version is better than no app
                }
            }
            try
            {
                string manifest = Path.Combine(root, "bbport-files.json");
                if (File.Exists(manifest))
                {
                    Match match = Regex.Match(File.ReadAllText(manifest, Encoding.UTF8), "\"version\"\\s*:\\s*\"([^\"]+)\"");
                    if (match.Success) return match.Groups[1].Value.Trim();
                }
            }
            catch (Exception)
            {
                // same: the version is a nicety
            }
            return "";
        }

        private static void Require(BbportInfo info, string relative, string shown)
        {
            string path = Path.Combine(info.Root, relative);
            if (!File.Exists(path) && !Directory.Exists(path)) info.Missing.Add(shown);
        }

        /// <summary>The BBSC release bbport's seamless folder is (seamless\VERSION.txt: "... v1.0.0 ...").</summary>
        private static string ReadCoopRelease(string root)
        {
            try
            {
                string path = Path.Combine(root, CoopSchema.CoopReleaseFile);
                if (!File.Exists(path)) return "";
                Match match = Regex.Match(File.ReadAllText(path, Encoding.UTF8), @"v\d+\.\d+(?:\.\d+)?");
                return match.Success ? match.Value.TrimStart('v') : "";
            }
            catch (Exception)
            {
                return "";
            }
        }

        private static void Note(BbportInfo info, string relative, string shown)
        {
            if (!Directory.Exists(Path.Combine(info.Root, relative))) info.Notes.Add(shown);
        }

        /// <summary>
        /// A first guess at the port's folder, without asking. The launcher's own settings keep the game
        /// folder (...\CUSA03173) and the port often sits beside it, so that is tried first; then the few
        /// layouts a port is normally unpacked into; then a short walk of each fixed drive. Read only:
        /// BBRadmin never writes any of it, and the player can always pick the folder by hand.
        /// </summary>
        public static string GuessFolder()
        {
            string fromSettings = GuessFromSettings();
            if (fromSettings.Length > 0) return fromSettings;

            foreach (string candidate in Candidates())
            {
                if (HasPort(candidate)) return candidate;
            }

            // Last resort: a bounded look around each fixed drive, so an unusual location is still found.
            string[] skip = { "windows", "$recycle.bin", "program files", "program files (x86)", "programdata",
                              "system volume information", "recovery", "perflogs", "msocache", "appdata" };
            int budget = 6000;
            try
            {
                foreach (DriveInfo drive in DriveInfo.GetDrives())
                {
                    if (drive.DriveType != DriveType.Fixed || !drive.IsReady) continue;
                    string found = FindPort(drive.RootDirectory.FullName, 0, 2, skip, ref budget);
                    if (found.Length > 0) return found;
                    if (budget <= 0) break;
                }
            }
            catch (Exception error)
            {
                Log.Line("Não deu para procurar a pasta do bbport (" + error.Message + ").");
            }
            return "";
        }

        /// <summary>The places a port is normally unpacked into, on every fixed drive and in the profile.</summary>
        private static IEnumerable<string> Candidates()
        {
            List<string> places = new List<string>();
            string[] layouts =
            {
                @"Games\Bloodborne-Windows\bbport-windows", @"Games\Bloodborne-Windows",
                @"Games\bbport-windows", @"Bloodborne-Windows\bbport-windows",
                "bbport-windows", @"Games\bbport"
            };
            try
            {
                foreach (DriveInfo drive in DriveInfo.GetDrives())
                {
                    if (drive.DriveType != DriveType.Fixed || !drive.IsReady) continue;
                    foreach (string layout in layouts) places.Add(Path.Combine(drive.RootDirectory.FullName, layout));
                }
            }
            catch (Exception) { }
            string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            foreach (string layout in new[] { "bbport-windows", @"Downloads\bbport-windows", @"Documents\bbport-windows" })
            {
                places.Add(Path.Combine(profile, layout));
            }
            return places;
        }

        private static string FindPort(string folder, int depth, int maxDepth, string[] skip, ref int budget)
        {
            if (budget <= 0) return "";
            budget--;
            if (HasPort(folder)) return folder;
            if (depth >= maxDepth) return "";
            string[] children;
            try
            {
                children = Directory.GetDirectories(folder);
            }
            catch (Exception)
            {
                return "";
            }
            foreach (string child in children)
            {
                if (IsSkipped(Path.GetFileName(child), skip)) continue;
                string found = FindPort(child, depth + 1, maxDepth, skip, ref budget);
                if (found.Length > 0) return found;
                if (budget <= 0) return "";
            }
            return "";
        }

        private static bool IsSkipped(string name, string[] skip)
        {
            string lower = name.ToLowerInvariant();
            foreach (string word in skip)
            {
                if (lower == word) return true;
            }
            return false;
        }

        /// <summary>A folder is a port when the game's exe and the port's own engine are both in it.</summary>
        private static bool HasPort(string folder)
        {
            try
            {
                return File.Exists(Path.Combine(folder, CoopSchema.GameExe)) &&
                       File.Exists(Path.Combine(folder, @"bin\bb-probe.exe"));
            }
            catch (Exception)
            {
                return false;
            }
        }

        private static string GuessFromSettings()
        {
            try
            {
                string settings = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
                                               "bbport-launcher", "settings.json");
                if (!File.Exists(settings)) return "";
                string text = File.ReadAllText(settings, Encoding.UTF8);
                Match match = Regex.Match(text, "\"game_dir\"\\s*:\\s*\"((?:[^\"\\\\]|\\\\.)*)\"");
                if (!match.Success) return "";
                string gameDir = Regex.Unescape(match.Groups[1].Value);
                DirectoryInfo parent = Directory.GetParent(gameDir);
                if (parent == null) return "";
                if (HasPort(parent.FullName)) return parent.FullName;
                // a game folder moved elsewhere: look one level up for the port
                DirectoryInfo grand = parent.Parent;
                if (grand != null && HasPort(grand.FullName)) return grand.FullName;
            }
            catch (Exception error)
            {
                Log.Line("Não deu para adivinhar a pasta do bbport (" + error.Message + ").");
            }
            return "";
        }

        /// <summary>
        /// Whether a game (or the port's own engine) is running: the port keeps one game per PC and a start
        /// with another environment is ignored while it runs (run.py's check), so a co-op start closes it first.
        /// </summary>
        public static List<Process> RunningGame()
        {
            List<Process> found = new List<Process>();
            string[] names = { "Bloodborne", "bb-probe", "bb-probe-seamless" };
            foreach (string name in names)
            {
                try
                {
                    foreach (Process process in Process.GetProcessesByName(name))
                    {
                        if (!process.HasExited) found.Add(process);
                    }
                }
                catch (Exception)
                {
                    // a process that cannot be inspected is not a reason to stop
                }
            }
            return found;
        }

        /// <summary>Closes what RunningGame found, politely first. Returns how many were closed.</summary>
        public static int CloseGame(List<Process> processes)
        {
            int closed = 0;
            foreach (Process process in processes)
            {
                try
                {
                    if (!process.CloseMainWindow()) process.Kill();
                    if (process.WaitForExit(8000)) closed++;
                    else
                    {
                        process.Kill();
                        if (process.WaitForExit(4000)) closed++;
                    }
                }
                catch (Exception error)
                {
                    Log.Line("Não fechei um processo do jogo (" + error.Message + ").");
                }
            }
            return closed;
        }

        public static string Human(DateTime when)
        {
            return when.ToString("HH:mm:ss", CultureInfo.InvariantCulture);
        }
    }
}
