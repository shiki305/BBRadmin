using System.Collections.Generic;
using System.Globalization;

namespace BBRadmin.Core
{
    /// <summary>
    /// The leader's co-op rules: what the in-game team screen shows to everyone and only the leader
    /// changes (BBSC: Yharnam Together's host rules — bbport_yharnam.host_rules and custom_enemies).
    /// BBRadmin sends them in the co-op start's environment, exactly as the bbport launcher does, so the
    /// team screen has something to show. They are the leader's: the guests see them, not set them.
    ///
    /// The port's launcher keeps its own copy of the same choices on its Seamless co-op page
    /// (BbportLauncher), which is where the defaults come from.
    /// </summary>
    internal sealed class HostRules
    {
        public bool Rot;                 // 'rot': the world's layout turns
        public bool Travel;              // 'travel': the hunters move between worlds together
        public bool Vote;                // 'vote': travelling together is decided by a vote
        public bool Revive = true;       // off is 'norevive': a fallen hunter stays down
        public string Enemies = "off";   // off | auto | bit | on | much | custom
        public string Tempo = "";        // "" (normal) | aggressive | relentless
        public string CustomHp = "2";    // only read when Enemies is custom, each 1 to 10
        public string CustomDamage = "1.5";
        public string CustomPoise = "2";

        public HostRules Copy()
        {
            HostRules copy = new HostRules();
            copy.Rot = Rot;
            copy.Travel = Travel;
            copy.Vote = Vote;
            copy.Revive = Revive;
            copy.Enemies = Enemies;
            copy.Tempo = Tempo;
            copy.CustomHp = CustomHp;
            copy.CustomDamage = CustomDamage;
            copy.CustomPoise = CustomPoise;
            return copy;
        }

        /// <summary>
        /// SHADPS4_BLOODBORNE_HOST_RULES: the rules as one comma list, '' for none. The same words, in the
        /// same order, as bbport_yharnam.host_rules (the vote is part of travelling together).
        /// </summary>
        public string Text()
        {
            List<string> words = new List<string>();
            if (Rot) words.Add("rot");
            if (Travel)
            {
                words.Add("travel");
                if (Vote) words.Add("vote");
            }
            if (Enemies == "auto") words.Add("scale");
            if (Tempo == "aggressive" || Tempo == "relentless") words.Add(Tempo);
            if (!Revive) words.Add("norevive");
            return string.Join(",", words.ToArray());
        }

        /// <summary>
        /// SHADPS4_BLOODBORNE_HARDER_ENEMIES: "hp,damage,poise", '' for none (off, or auto where the
        /// 'scale' rule decides instead). The presets are bbport_yharnam.ENEMIES.
        /// </summary>
        public string EnemiesText()
        {
            switch (Enemies)
            {
                case "bit": return "1.5,1.25,1.5";
                case "on": return "3,1.5,3";
                case "much": return "5,2,5";
                case "custom":
                    return Multiplier(CustomHp, 2) + "," + Multiplier(CustomDamage, 1.5) + "," +
                           Multiplier(CustomPoise, 2);
                default: return "";
            }
        }

        /// <summary>One line for the window and the log: what the team will see.</summary>
        public string Describe()
        {
            string rules = Text();
            string enemies = EnemiesText();
            if (rules.Length == 0 && enemies.Length == 0) return "nenhuma (padrão)";
            string shown = rules.Length > 0 ? rules : "sem outras regras";
            if (enemies.Length > 0) shown += " · inimigos " + enemies;
            return shown;
        }

        private static string Multiplier(string raw, double fallback)
        {
            double value;
            string text = (raw ?? "").Trim().Replace(',', '.');
            if (!double.TryParse(text, NumberStyles.Float, CultureInfo.InvariantCulture, out value)) value = fallback;
            if (value < 1) value = 1;
            if (value > 10) value = 10;   // the engine's range
            return value.ToString("0.###", CultureInfo.InvariantCulture);
        }
    }
}
