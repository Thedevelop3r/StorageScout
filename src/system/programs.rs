//! Installed program inventory.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ProgramSource {
    /// HKLM 64-bit
    Machine,
    /// HKLM WOW6432Node
    Machine32,
    /// HKCU
    User,
    /// Microsoft Store / AppX package
    Store,
    /// Linux .desktop / macOS .app
    #[cfg_attr(windows, allow(dead_code))]
    Other,
}

impl ProgramSource {
    pub fn label(self) -> &'static str {
        match self {
            ProgramSource::Machine => "All users (64-bit)",
            ProgramSource::Machine32 => "All users (32-bit)",
            ProgramSource::User => "Current user",
            ProgramSource::Store => "Store app",
            ProgramSource::Other => "Application",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Program {
    pub name: String,
    pub version: String,
    pub publisher: String,
    /// YYYYMMDD as stored by Windows, or empty.
    pub install_date: String,
    pub install_location: Option<PathBuf>,
    /// Size reported by the installer (bytes).
    pub estimated_size: u64,
    pub uninstall_string: Option<String>,
    pub source: ProgramSource,
    pub system_component: bool,
}

impl Program {
    pub fn install_date_display(&self) -> String {
        let d = &self.install_date;
        if d.len() == 8 && d.chars().all(|c| c.is_ascii_digit()) {
            format!("{}-{}-{}", &d[0..4], &d[4..6], &d[6..8])
        } else {
            d.clone()
        }
    }
}

/// Load installed programs (blocking; call from a background thread).
pub fn list_installed() -> Vec<Program> {
    #[cfg(windows)]
    {
        let mut v = windows::uninstall_entries();
        v.extend(windows::store_packages());
        dedupe(v)
    }
    #[cfg(target_os = "linux")]
    {
        dedupe(unix::desktop_entries())
    }
    #[cfg(target_os = "macos")]
    {
        dedupe(unix::mac_apps())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Vec::new()
    }
}

#[allow(dead_code)]
fn dedupe(mut v: Vec<Program>) -> Vec<Program> {
    v.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.version.cmp(&b.version))
    });
    v.dedup_by(|a, b| {
        a.name.eq_ignore_ascii_case(&b.name) && a.version == b.version && a.source == b.source
    });
    v
}

#[cfg(windows)]
mod windows {
    use super::{Program, ProgramSource};
    use std::path::PathBuf;
    use winreg::RegKey;
    use winreg::enums::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
    };

    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

    pub fn uninstall_entries() -> Vec<Program> {
        let mut out = Vec::new();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let sources = [
            (&hklm, KEY_READ | KEY_WOW64_64KEY, ProgramSource::Machine),
            (&hklm, KEY_READ | KEY_WOW64_32KEY, ProgramSource::Machine32),
            (&hkcu, KEY_READ, ProgramSource::User),
        ];
        for (hive, flags, source) in sources {
            let Ok(key) = hive.open_subkey_with_flags(UNINSTALL, flags) else {
                continue;
            };
            for sub in key.enum_keys().flatten() {
                let Ok(k) = key.open_subkey_with_flags(&sub, flags) else {
                    continue;
                };
                let name: String = k.get_value("DisplayName").unwrap_or_default();
                if name.trim().is_empty() {
                    continue;
                }
                // Updates / patches attached to a parent product
                let parent: Result<String, _> = k.get_value("ParentKeyName");
                if parent.is_ok() {
                    continue;
                }
                let release_type: String = k.get_value("ReleaseType").unwrap_or_default();
                if release_type.contains("Update") || release_type.contains("Hotfix") {
                    continue;
                }
                let system_component: u32 = k.get_value("SystemComponent").unwrap_or(0);
                let size_kb: u32 = k.get_value("EstimatedSize").unwrap_or(0);
                let location: String = k.get_value("InstallLocation").unwrap_or_default();
                let location = clean_path(&location);
                // The interactive uninstaller is preferred over QuietUninstallString.
                let uninstall: String = k.get_value("UninstallString").unwrap_or_default();
                let icon_dir = || -> Option<PathBuf> {
                    let icon: String = k.get_value("DisplayIcon").ok()?;
                    let p = PathBuf::from(clean_path(icon.split(',').next()?)?);
                    let parent = p.parent()?.to_path_buf();
                    if parent.as_os_str().is_empty() {
                        None
                    } else {
                        Some(parent)
                    }
                };
                out.push(Program {
                    name: name.trim().to_owned(),
                    version: k.get_value("DisplayVersion").unwrap_or_default(),
                    publisher: k
                        .get_value::<String, _>("Publisher")
                        .unwrap_or_default()
                        .trim()
                        .to_owned(),
                    install_date: k.get_value("InstallDate").unwrap_or_default(),
                    install_location: location.map(PathBuf::from).or_else(icon_dir),
                    estimated_size: size_kb as u64 * 1024,
                    uninstall_string: if uninstall.trim().is_empty() {
                        None
                    } else {
                        Some(uninstall)
                    },
                    source,
                    system_component: system_component == 1,
                });
            }
        }
        out
    }

    fn clean_path(s: &str) -> Option<String> {
        let t = s.trim().trim_matches('"').trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_owned())
        }
    }

    /// Store / AppX packages registered for the current user.
    pub fn store_packages() -> Vec<Program> {
        const REPO: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
        let mut out = Vec::new();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(key) = hkcu.open_subkey_with_flags(REPO, KEY_READ) else {
            return out;
        };
        for full_name in key.enum_keys().flatten() {
            let Ok(k) = key.open_subkey(&full_name) else {
                continue;
            };
            let display: String = k.get_value("DisplayName").unwrap_or_default();
            let location: String = k.get_value("PackageRootFolder").unwrap_or_default();
            // full name: Name_Version_Arch_ResourceId_PublisherId
            let mut parts = full_name.split('_');
            let id_name = parts.next().unwrap_or_default().to_owned();
            let version = parts.next().unwrap_or_default().to_owned();
            // Skip frameworks / resource packs.
            if full_name.contains("_neutral_split")
                || id_name.starts_with("Microsoft.VCLibs")
                || id_name.starts_with("Microsoft.NET.Native")
                || id_name.starts_with("Microsoft.UI.Xaml")
            {
                continue;
            }
            let name = if display.is_empty()
                || display.starts_with("@{")
                || display.starts_with("ms-resource")
            {
                id_name.clone()
            } else {
                display
            };
            let publisher = id_name.split('.').next().unwrap_or_default().to_owned();
            out.push(Program {
                name,
                version,
                publisher,
                install_date: String::new(),
                install_location: if location.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(location))
                },
                estimated_size: 0,
                uninstall_string: None,
                source: ProgramSource::Store,
                system_component: false,
            });
        }
        out
    }
}

#[cfg(unix)]
#[allow(dead_code)]
mod unix {
    use super::{Program, ProgramSource};
    use std::path::PathBuf;

    pub fn desktop_entries() -> Vec<Program> {
        let mut dirs = vec![
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join(".local/share/applications"));
        }
        let mut out = Vec::new();
        for d in dirs {
            let Ok(rd) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "desktop")
                    && let Ok(text) = std::fs::read_to_string(&p)
                {
                    let get = |k: &str| {
                        text.lines()
                            .find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('=')))
                            .unwrap_or_default()
                            .to_owned()
                    };
                    let name = get("Name");
                    if name.is_empty() || get("NoDisplay") == "true" {
                        continue;
                    }
                    out.push(Program {
                        name,
                        version: get("Version"),
                        publisher: String::new(),
                        install_date: String::new(),
                        install_location: Some(p.clone()),
                        estimated_size: 0,
                        uninstall_string: None,
                        source: ProgramSource::Other,
                        system_component: false,
                    });
                }
            }
        }
        out
    }

    pub fn mac_apps() -> Vec<Program> {
        let mut out = Vec::new();
        for d in ["/Applications", "/System/Applications"] {
            let Ok(rd) = std::fs::read_dir(d) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "app") {
                    out.push(Program {
                        name: p
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        version: String::new(),
                        publisher: String::new(),
                        install_date: String::new(),
                        install_location: Some(p.clone()),
                        estimated_size: 0,
                        uninstall_string: None,
                        source: ProgramSource::Other,
                        system_component: d.starts_with("/System"),
                    });
                }
            }
        }
        out
    }
}
