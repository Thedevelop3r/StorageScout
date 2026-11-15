//! Small settings file kept in `%APPDATA%\PC Scanner\settings.json`.
//!
//! It only holds preferences (theme, accent colour, update choices). Scan results are never
//! written here.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Dark theme (true) or light theme (false).
    pub dark: bool,
    /// Accent colour name, one of `ui::theme::ACCENTS`.
    pub accent: String,
    /// Look for a new release on GitHub when the program starts (at most once a day).
    pub check_updates: bool,
    /// The user answered the first-start question about update checks. Until then no
    /// network request is made.
    pub update_choice_made: bool,
    /// Unix time of the last successful update check.
    pub last_update_check: Option<i64>,
    /// Version the user chose to skip ("0.3.0").
    pub skipped_version: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark: true,
            accent: "Cyan".into(),
            check_updates: true,
            update_choice_made: false,
            last_update_check: None,
            skipped_version: None,
        }
    }
}

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("PC Scanner").join("settings.json"))
}

impl Settings {
    /// Load the settings file; missing or unreadable files give the defaults.
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save the settings file. Errors are returned so the caller can show them.
    pub fn save(&self) -> Result<(), String> {
        let p = path().ok_or("APPDATA is not set")?;
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write to a temporary file first so a crash never leaves a half-written file.
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str(r#"{"dark": false, "future": 1}"#).unwrap();
        assert!(!s.dark);
        assert!(s.check_updates);
        assert_eq!(s.accent, "Cyan");
    }
}
