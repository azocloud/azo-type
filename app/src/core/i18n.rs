//! Cross-platform locale normalization, system language detection, and native display names.

#[cfg(test)]
mod tests;

/// Normalizes an RFC 5646 / BCP 47 or POSIX locale identifier (e.g. `"de-DE"`, `"en_US"`, `"de"`)
/// into its lowercase primary language subtag (e.g. `"de"`, `"en"`).
pub fn normalize_locale_tag(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "en".to_string();
    }
    let primary = trimmed
        .split(['-', '_', '.'])
        .next()
        .unwrap_or("en")
        .trim();
    if primary.is_empty() {
        "en".to_string()
    } else {
        primary.to_ascii_lowercase()
    }
}

/// Resolves an optional raw locale string against a list of available locales,
/// falling back to `"en"` if no match is found.
pub fn resolve_locale(detected: Option<&str>, available: &[&str]) -> String {
    if let Some(raw) = detected {
        let norm = normalize_locale_tag(raw);
        if available.contains(&norm.as_str()) {
            return norm;
        }
    }
    "en".to_string()
}

/// Detects the system language via the operating system locale, matching against available locales.
pub fn detect_system_locale(available: &[&str]) -> String {
    let os_locale = sys_locale::get_locale();
    resolve_locale(os_locale.as_deref(), available)
}

/// Returns the native human-readable display name for a locale code (e.g. `"en"` -> `"English"`).
pub fn locale_native_name(locale: &str) -> &'static str {
    match locale {
        "en" => "English",
        "de" => "Deutsch",
        "fr" => "Français",
        "es" => "Español",
        "it" => "Italiano",
        "ja" => "日本語",
        "zh" | "zh-CN" => "简体中文",
        "zh-TW" => "繁體中文",
        _ => "English",
    }
}
