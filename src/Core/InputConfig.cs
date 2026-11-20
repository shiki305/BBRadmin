using System;
using System.IO;
using System.Text;

namespace BBRadmin.Core
{
    /// <summary>
    /// <bbport>\input.ini — the controls file (shadPS4 syntax: "output = input", one input per line).
    /// The port's own launcher keeps it on its Controls page, whose "You play with" choice is the same
    /// one thing BBRadmin edits here:
    ///
    ///   Teclado e mouse  the mouse camera on  (the one line `mouse_to_joystick = right`)
    ///   Controle         the mouse camera off (that line commented out)
    ///
    /// Everything else in the file is the player's own keys and controller buttons, and both work side
    /// by side whatever this is: the switch only decides whether moving the mouse turns the camera (and
    /// captures the pointer). During play F7 toggles the mouse and F8 reloads the file.
    ///
    /// Only that one line is ever touched. The file is backed up once, under
    /// input.ini.bbradmin-bak, so the player's own file can always be put back.
    /// </summary>
    internal static class InputConfig
    {
        public const string Keyboard = "keyboard";    // the port's own word for "teclado e mouse"
        public const string Controller = "controller";

        private const string MouseKey = "mouse_to_joystick";
        private const string MouseOn = "right";
        private const string BackupSuffix = ".bbradmin-bak";
        private static readonly UTF8Encoding Utf8NoBom = new UTF8Encoding(false);

        public static string PathIn(BbportInfo port)
        {
            return Path.Combine(port.Root, CoopSchema.InputFile);
        }

        public static string BackupPathIn(BbportInfo port)
        {
            return PathIn(port) + BackupSuffix;
        }

        /// <summary>
        /// What the file says: Keyboard (the mouse camera is on), Controller (it is not), or '' when
        /// there is no file to read.
        /// </summary>
        public static string Current(BbportInfo port)
        {
            string text = Read(port);
            if (text.Length == 0) return "";
            return MouseCameraOn(text) ? Keyboard : Controller;
        }

        /// <summary>One line for the window and the log: what the file is set to.</summary>
        public static string Describe(BbportInfo port)
        {
            string mode = Current(port);
            if (mode.Length == 0) return "sem " + CoopSchema.InputFile + " (o jogo usa os controles padrão dele)";
            if (mode == Keyboard) return "teclado e mouse (o mouse gira a câmera; F7 liga/desliga no jogo)";
            return "controle (o mouse não mexe a câmera; as teclas continuam funcionando)";
        }

        /// <summary>
        /// Makes the file match <paramref name="mode"/> ("keyboard" or "controller"). Touches the one
        /// `mouse_to_joystick` line only, and backs the file up once before changing it. Never throws.
        /// </summary>
        public static string Apply(BbportInfo port, string mode, out string error)
        {
            error = "";
            string path = PathIn(port);
            bool on = mode == Keyboard;
            string wanted = on ? "Teclado e mouse" : "Controle";
            string wantedLong = on ? "teclado e mouse (o mouse gira a câmera)" : "controle (o mouse não mexe a câmera)";

            if (!File.Exists(path))
            {
                return "Não mexi nos controles: " + CoopSchema.InputFile + " não está em " + port.Root +
                       " (o jogo usa os controles padrão dele).";
            }

            try
            {
                byte[] raw = File.ReadAllBytes(path);
                bool bom = raw.Length >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF;
                string text = new UTF8Encoding(false).GetString(raw, bom ? 3 : 0, raw.Length - (bom ? 3 : 0));

                if (MouseCameraOn(text) == on)
                {
                    return "Controles já estavam em " + wanted + " (nada mudou).";
                }

                string[] lines = text.Split('\n');
                bool found = false;
                for (int i = 0; i < lines.Length; i++)
                {
                    string ending = lines[i].EndsWith("\r") ? "\r" : "";
                    string body = ending.Length > 0 ? lines[i].Substring(0, lines[i].Length - 1) : lines[i];
                    if (KeyOf(body) != MouseKey) continue;
                    string value = ValueOf(body);
                    lines[i] = (on ? MouseKey + " = " + value : "# " + MouseKey + " = " + value) + ending;
                    found = true;
                }
                if (!found && on)
                {
                    // No such line at all (a file that never had one): add it at the end, marked as ours.
                    string added = "# BBRadmin: o mouse gira a câmera (F7 liga/desliga no jogo)\n" +
                                   MouseKey + " = " + MouseOn + "\n";
                    text = text.Length > 0 && !text.EndsWith("\n") ? text + "\n" : text;
                    text += added;
                    lines = null;
                }

                BackupOnce(path);
                if (lines != null) text = string.Join("\n", lines);
                File.WriteAllText(path, text, bom ? new UTF8Encoding(true) : Utf8NoBom);
                return "Controles agora em " + wantedLong + " (" + CoopSchema.InputFile + ").";
            }
            catch (Exception exception)
            {
                error = "Não escrevi " + CoopSchema.InputFile + ": " + exception.Message;
                return error;
            }
        }

        /// <summary>Puts the file back as it was before BBRadmin touched it (the one backup it keeps).</summary>
        public static string Restore(BbportInfo port, out string error)
        {
            error = "";
            string path = PathIn(port);
            string backup = BackupPathIn(port);
            try
            {
                if (!File.Exists(backup)) return "Nada a restaurar: o BBRadmin ainda não mudou os controles.";
                File.Copy(backup, path, true);
                File.Delete(backup);
                return "Controles restaurados como estavam antes do BBRadmin.";
            }
            catch (Exception exception)
            {
                error = "Não restaurei os controles: " + exception.Message;
                return error;
            }
        }

        // ---- the one line's grammar ---------------------------------------------------------------

        /// <summary>True when the file has an active (not commented) mouse_to_joystick line.</summary>
        private static bool MouseCameraOn(string text)
        {
            foreach (string raw in text.Split('\n'))
            {
                string body = raw.TrimEnd('\r').TrimStart();
                if (body.StartsWith("#")) continue;   // commented out: the mouse camera is off
                if (KeyOf(body) == MouseKey) return true;
            }
            return false;
        }

        /// <summary>The setting name a line carries, whether it is commented out or not.</summary>
        private static string KeyOf(string body)
        {
            string text = body.TrimStart();
            while (text.StartsWith("#")) text = text.Substring(1).TrimStart();
            int split = text.IndexOf('=');
            return (split < 0 ? text : text.Substring(0, split)).Trim();
        }

        /// <summary>The value a line carries, whether it is commented out or not ("right" when it has none).</summary>
        private static string ValueOf(string body)
        {
            int split = body.IndexOf('=');
            if (split < 0) return MouseOn;
            string value = body.Substring(split + 1).Trim();
            return value.Length > 0 ? value : MouseOn;
        }

        private static string Read(BbportInfo port)
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

        /// <summary>Keeps the player's own file once, so Restore brings back the original, not a later edit.</summary>
        private static void BackupOnce(string path)
        {
            string target = path + BackupSuffix;
            if (File.Exists(target)) return;
            try
            {
                File.Copy(path, target, true);
            }
            catch (Exception error)
            {
                Log.Line("Backup do " + CoopSchema.InputFile + " não feito (" + error.Message + ").");
            }
        }
    }
}
