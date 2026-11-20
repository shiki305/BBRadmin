using System;
using System.Globalization;
using System.IO;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>BBRadmin's own log: %LOCALAPPDATA%\BBRadmin\bbradmin.log (kept under ~512 KB, then rotated once).</summary>
    internal static class Log
    {
        private const long MaxBytes = 512 * 1024;
        private static readonly object Gate = new object();

        public static string Folder
        {
            get { return AppConfig.Folder; }
        }

        public static string FilePath
        {
            get { return Path.Combine(Folder, "bbradmin.log"); }
        }

        public static void Line(string message)
        {
            try
            {
                lock (Gate)
                {
                    Directory.CreateDirectory(Folder);
                    FileInfo info = new FileInfo(FilePath);
                    if (info.Exists && info.Length > MaxBytes)
                    {
                        string previous = FilePath + ".1";
                        if (File.Exists(previous)) File.Delete(previous);
                        File.Move(FilePath, previous);
                    }
                    string stamp = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture);
                    File.AppendAllText(FilePath, stamp + "  " + message + Environment.NewLine, new UTF8Encoding(false));
                }
            }
            catch (Exception)
            {
                // a log that cannot be written must never break the app
            }
        }

        public static string Tail(int lines)
        {
            try
            {
                if (!File.Exists(FilePath)) return "";
                string[] all = File.ReadAllLines(FilePath, Encoding.UTF8);
                int from = Math.Max(0, all.Length - lines);
                StringBuilder text = new StringBuilder();
                for (int i = from; i < all.Length; i++) text.AppendLine(all[i]);
                return text.ToString();
            }
            catch (Exception)
            {
                return "";
            }
        }
    }
}
