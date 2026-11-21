//! The leader's co-op rules (BBSC: Yharnam Together's SHADPS4_BLOODBORNE_HOST_RULES and
//! SHADPS4_BLOODBORNE_HARDER_ENEMIES).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRules {
    pub rot: bool,
    pub travel: bool,
    pub vote: bool,
    pub revive: bool,
    pub enemies: String,       // off | auto | bit | on | much | custom
    pub tempo: String,         // "" | aggressive | relentless
    pub custom_hp: String,     // 1..=10
    pub custom_damage: String, // 1..=10
    pub custom_poise: String,  // 1..=10
}

impl Default for HostRules {
    fn default() -> Self {
        Self {
            rot: false,
            travel: false,
            vote: false,
            revive: true,
            enemies: "off".to_string(),
            tempo: String::new(),
            custom_hp: "2".to_string(),
            custom_damage: "1.5".to_string(),
            custom_poise: "2".to_string(),
        }
    }
}

impl HostRules {
    /// SHADPS4_BLOODBORNE_HOST_RULES: comma list, "" for none.
    pub fn text(&self) -> String {
        let mut words = Vec::new();
        if self.rot {
            words.push("rot");
        }
        if self.travel {
            words.push("travel");
            if self.vote {
                words.push("vote");
            }
        }
        if self.enemies == "auto" {
            words.push("scale");
        }
        if self.tempo == "aggressive" || self.tempo == "relentless" {
            words.push(self.tempo.as_str());
        }
        if !self.revive {
            words.push("norevive");
        }
        words.join(",")
    }

    /// SHADPS4_BLOODBORNE_HARDER_ENEMIES: "hp,damage,poise", "" for none.
    pub fn enemies_text(&self) -> String {
        match self.enemies.as_str() {
            "bit" => "1.5,1.25,1.5".to_string(),
            "on" => "3,1.5,3".to_string(),
            "much" => "5,2,5".to_string(),
            "custom" => format!(
                "{},{},{}",
                multiplier(&self.custom_hp, 2.0),
                multiplier(&self.custom_damage, 1.5),
                multiplier(&self.custom_poise, 2.0),
            ),
            _ => String::new(),
        }
    }

    pub fn describe(&self) -> String {
        self.describe_for(crate::core::i18n::Lang::PtBr)
    }

    pub fn describe_for(&self, lang: crate::core::i18n::Lang) -> String {
        use crate::core::i18n::t;
        let rules = self.text();
        let enemies = self.enemies_text();
        if rules.is_empty() && enemies.is_empty() {
            return t(lang, "nenhuma (padrão)", "none (default)").to_string();
        }
        let mut shown = if !rules.is_empty() {
            rules
        } else {
            t(lang, "sem outras regras", "no other rules").to_string()
        };
        if !enemies.is_empty() {
            shown.push_str(t(lang, " · inimigos ", " · enemies "));
            shown.push_str(&enemies);
        }
        shown
    }
}

fn multiplier(raw: &str, fallback: f64) -> String {
    let text = raw.trim().replace(',', ".");
    let mut value = text.parse::<f64>().unwrap_or(fallback);
    if !value.is_finite() {
        value = fallback;
    }
    value = value.clamp(1.0, 10.0);
    // Format up to 3 decimal places without trailing zeros (like C# "0.###").
    let formatted = format!("{value:.3}");
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_rules_formatting() {
        let mut r = HostRules::default();
        assert_eq!(r.text(), "");
        assert_eq!(r.enemies_text(), "");

        r.travel = true;
        r.revive = false;
        r.enemies = "auto".into();
        assert_eq!(r.text(), "travel,scale,norevive");
        assert_eq!(r.enemies_text(), "");

        r.enemies = "custom".into();
        r.custom_hp = "2,5".into();
        r.custom_damage = "15".into();
        r.custom_poise = "0.5".into();
        assert_eq!(r.enemies_text(), "2.5,10,1");
    }
}
