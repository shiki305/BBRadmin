//! UI localization: Português (BR) and English (US), with automatic Windows language detection.

use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;

const LANG_PORTUGUESE: u16 = 0x16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    PtBr,
    EnUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangMode {
    Auto,
    PtBr,
    EnUs,
}

impl LangMode {
    pub const ALL: &[LangMode] = &[LangMode::Auto, LangMode::PtBr, LangMode::EnUs];

    pub fn from_config(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "pt" | "pt-br" | "pt_br" => Self::PtBr,
            "en" | "en-us" | "en_us" => Self::EnUs,
            _ => Self::Auto,
        }
    }

    pub fn as_config(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::PtBr => "pt-br",
            Self::EnUs => "en-us",
        }
    }

    pub fn resolve(self) -> Lang {
        match self {
            Self::Auto => detect_system_lang(),
            Self::PtBr => Lang::PtBr,
            Self::EnUs => Lang::EnUs,
        }
    }

    pub fn label(self, ui_lang: Lang) -> &'static str {
        match self {
            Self::Auto => t(ui_lang, "Automático (Windows)", "Automatic (Windows)"),
            Self::PtBr => "Português (BR)",
            Self::EnUs => "English (US)",
        }
    }
}

pub fn detect_system_lang() -> Lang {
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary = lang_id & 0x03FF;
    if primary == LANG_PORTUGUESE {
        Lang::PtBr
    } else {
        Lang::EnUs
    }
}

#[inline]
pub const fn t<'a>(lang: Lang, pt: &'a str, en: &'a str) -> &'a str {
    match lang {
        Lang::PtBr => pt,
        Lang::EnUs => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_mode_roundtrip() {
        assert_eq!(LangMode::from_config("en-us"), LangMode::EnUs);
        assert_eq!(LangMode::from_config("pt-br"), LangMode::PtBr);
        assert_eq!(LangMode::from_config("auto"), LangMode::Auto);
        assert_eq!(LangMode::from_config(""), LangMode::Auto);
        assert_eq!(t(Lang::PtBr, "Olá", "Hello"), "Olá");
        assert_eq!(t(Lang::EnUs, "Olá", "Hello"), "Hello");
    }
}
