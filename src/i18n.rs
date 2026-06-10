//! Lightweight in-app internationalization (English + Simplified Chinese).
//!
//! The UI is keyed by its **English source string**: `t("Connect")` returns the
//! Chinese translation when the effective language is Chinese, and the English
//! text itself otherwise (or when a translation is missing). This keeps call
//! sites readable and makes English a guaranteed fallback.
//!
//! The effective language lives in a process-global atomic, set once per render
//! from the persisted setting (see `main::root`). Because the reactor re-renders
//! from the root and rebuilds the whole widget tree on any state change, setting
//! the atomic *synchronously at the top of the root* — before children run —
//! means every `t()` in that render already sees the new language. The
//! re-render itself is driven by the `Store` prop changing, exactly like any
//! other setting.
//!
//! Translations are read from any thread (the tray builds its menu/tooltip off
//! the UI thread), so the atomic is the single source of truth.

use std::sync::atomic::{AtomicU8, Ordering};

mod zh;

/// The user-facing language preference (persisted in `Settings`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
pub enum Language {
    /// Follow the operating system's UI language (English unless it is Chinese).
    #[default]
    System,
    English,
    Chinese,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::System, Language::English, Language::Chinese];

    /// Label for the language picker. The "system" option follows the current
    /// language; the concrete languages always read in their own script.
    pub fn label(self) -> String {
        match self {
            Language::System => t("Follow system language"),
            Language::English => "English".to_string(),
            Language::Chinese => "中文".to_string(),
        }
    }

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0) as i32
    }

    pub fn from_index(i: i32) -> Self {
        *Self::ALL.get(i.max(0) as usize).unwrap_or(&Language::System)
    }
}

// Effective language codes stored in the atomic.
const EN: u8 = 0;
const ZH: u8 = 1;

static EFFECTIVE: AtomicU8 = AtomicU8::new(EN);

/// Resolve a preference to a concrete language, detecting the OS locale for
/// [`Language::System`].
fn resolve(lang: Language) -> u8 {
    match lang {
        Language::English => EN,
        Language::Chinese => ZH,
        Language::System => detect_os(),
    }
}

/// Chinese if the OS UI locale starts with `zh` (zh-CN, zh-Hans, zh-TW, …),
/// English otherwise.
fn detect_os() -> u8 {
    let is_zh = sys_locale::get_locale()
        .map(|l| l.to_ascii_lowercase().starts_with("zh"))
        .unwrap_or(false);
    if is_zh { ZH } else { EN }
}

/// Apply the preference to the global. Call once per render from the root.
pub fn set(lang: Language) {
    EFFECTIVE.store(resolve(lang), Ordering::Relaxed);
}

/// Whether the effective language is Chinese. Used to remount the pages that
/// host translated `ComboBox`es when the language flips (see `ui::body_view`).
pub fn is_zh() -> bool {
    EFFECTIVE.load(Ordering::Relaxed) == ZH
}

/// Translate a (possibly dynamic) UI string. Returns an owned `String` so it
/// composes with both `impl Into<String>` widget setters and `format!`.
pub fn t(s: &str) -> String {
    if is_zh()
        && let Some(z) = zh::zh(s)
    {
        return z.to_string();
    }
    s.to_string()
}

/// Translate a `&'static str` to a `&'static str` — for `enum::label()` methods
/// that must keep that return type.
pub fn ts(s: &'static str) -> &'static str {
    if is_zh()
        && let Some(z) = zh::zh(s)
    {
        return z;
    }
    s
}

/// Translate a template containing `{n}`, then substitute the count. The English
/// key is the template itself, e.g. `tn("{n} peers", 3)`.
pub fn tn(template: &str, n: usize) -> String {
    t(template).replace("{n}", &n.to_string())
}
