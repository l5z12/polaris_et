//! In-app internationalization (English + Simplified Chinese).
//!
//! UI strings are referenced by **namespaced keys** (e.g. `tray.show_polaris`,
//! `network.general.profile_name`). Translations live in JSON locale files under
//! `locales/`, embedded at build time; `en.json` is the source of truth and the
//! fallback for any key missing from another locale.
//!
//! The effective language is a process-global atomic, set once per render from
//! the persisted setting (see `main::root`). Because the reactor re-renders the
//! whole tree from the root on any state change, setting the atomic
//! synchronously at the top of the root makes every `t()` in that render see the
//! new language; the re-render itself is driven by the `Store` prop changing,
//! like any other setting. Translations are read from any thread (the tray
//! builds its menu/tooltip off the UI thread).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

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
            Language::System => t("settings.language_follow"),
            Language::English => "English".to_string(),
            Language::Chinese => "中文".to_string(),
        }
    }

    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0) as i32
    }

    pub fn from_index(i: i32) -> Self {
        *Self::ALL
            .get(i.max(0) as usize)
            .unwrap_or(&Language::System)
    }
}

// Effective language codes stored in the atomic.
const EN: u8 = 0;
const ZH: u8 = 1;

static EFFECTIVE: AtomicU8 = AtomicU8::new(EN);

static EN_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
static ZH_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();

fn en_map() -> &'static HashMap<String, String> {
    EN_MAP.get_or_init(|| load(include_str!("../locales/en.json")))
}
fn zh_map() -> &'static HashMap<String, String> {
    ZH_MAP.get_or_init(|| load(include_str!("../locales/zh.json")))
}

/// Parse a locale JSON document and flatten its nested objects into dotted keys.
fn load(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    match serde_json::from_str::<serde_json::Value>(src) {
        Ok(v) => flatten(String::new(), &v, &mut out),
        Err(e) => {
            // The locale files ship with the binary, so this is a build-time
            // mistake; surface it but don't crash the UI.
            debug_assert!(false, "invalid locale JSON: {e}");
        }
    }
    out
}

fn flatten(prefix: String, v: &serde_json::Value, out: &mut HashMap<String, String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(key, val, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix, s.clone());
        }
        _ => {}
    }
}

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

/// Translate a key to an owned string.
///
/// Unknown keys are returned **verbatim** rather than panicking: the translating
/// helpers (`card`, `stat`, `my_node_chips`'s chip labels, …) are also handed
/// already-built dynamic strings — a section title like `"My net (3 peers)"` or
/// a listener label `"Listener 1"` — which must pass through unchanged. A real
/// missing key therefore surfaces as the raw key in the UI (and is caught by the
/// `locales` key test), never a crash.
pub fn t(key: &str) -> String {
    if is_zh()
        && let Some(s) = zh_map().get(key)
    {
        return s.clone();
    }
    match en_map().get(key) {
        Some(s) => s.clone(),
        None => key.to_string(),
    }
}

/// Translate a `&'static str` key to a `&'static str` — for `enum::label()`
/// methods that must keep that return type. Unknown keys return themselves.
pub fn ts(key: &'static str) -> &'static str {
    if is_zh()
        && let Some(s) = zh_map().get(key)
    {
        return s;
    }
    en_map().get(key).map(String::as_str).unwrap_or(key)
}

/// Translate a key whose value contains `{n}`, then substitute the count, e.g.
/// `tn("peers.count", 3)`.
pub fn tn(key: &str, n: usize) -> String {
    t(key).replace("{n}", &n.to_string())
}
