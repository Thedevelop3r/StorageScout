//! File system actions: open, reveal, copy, trash, delete, uninstall.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};

use crate::model::NodeId;

/// Open a file or folder with the default application.
pub fn open_path(path: &Path) -> Result<(), String> {
    open::that_detached(path).map_err(|e| e.to_string())
}

/// Show the item selected in the system file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let arg = format!("/select,\"{}\"", path.display());
        Command::new("explorer")
            .raw_arg(arg)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
        open::that_detached(target).map_err(|e| e.to_string())
    }
}

/// Paths that must never be deleted from this app.
pub fn protection_reason(path: &Path) -> Option<&'static str> {
    let comps = path.components().count();
    if comps <= 1 || path.parent().is_none() {
        return Some("drive / file system root");
    }
    let lower = path.to_string_lossy().to_lowercase().replace('/', "\\");
    let lower = lower.trim_end_matches('\\');
    #[cfg(windows)]
    {
        let sys_root = std::env::var("SystemRoot")
            .unwrap_or_else(|_| "C:\\Windows".into())
            .to_lowercase();
        let protected_exact = [
            sys_root.clone(),
            format!("{}\\system32", sys_root),
            "c:\\program files".into(),
            "c:\\program files (x86)".into(),
            "c:\\programdata".into(),
            "c:\\users".into(),
            "c:\\boot".into(),
            "c:\\recovery".into(),
        ];
        if protected_exact.iter().any(|p| p == lower) {
            return Some("essential system folder");
        }
        if let Ok(profile) = std::env::var("USERPROFILE")
            && profile.to_lowercase() == lower
        {
            return Some("your user profile folder");
        }
        let critical_files = ["pagefile.sys", "hiberfil.sys", "swapfile.sys", "bootmgr"];
        if let Some(name) = path.file_name() {
            let n = name.to_string_lossy().to_lowercase();
            let at_root = path.parent().is_some_and(|p| p.parent().is_none());
            if at_root && critical_files.contains(&n.as_str()) {
                return Some("system managed file (change it in Windows settings)");
            }
        }
        let sys32 = format!("{}\\system32\\", sys_root);
        let winsxs = format!("{}\\winsxs", sys_root);
        if lower.starts_with(&sys32) || lower.starts_with(&winsxs) {
            return Some("inside the Windows system directory");
        }
    }
    #[cfg(unix)]
    {
        let protected = ["/bin", "/sbin", "/usr", "/etc", "/lib", "/lib64", "/boot", "/home", "/System", "/Users"];
        if protected.iter().any(|p| p.to_lowercase() == lower) {
            return Some("essential system folder");
        }
    }
    let _ = lower;
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    RecycleBin,
    Permanent,
}

#[derive(Debug, Clone)]
pub struct DeleteTarget {
    pub id: NodeId,
    pub path: PathBuf,
    /// Delete only the folder contents, keep the folder itself.
    pub contents_only: bool,
    pub size: u64,
}

pub struct DeleteOutcome {
    pub target: DeleteTarget,
    pub result: Result<(), String>,
    /// For contents-only targets: the children that could not be removed (by name).
    pub kept_children: Vec<String>,
}

/// A delete batch running on a background thread.
pub struct DeleteJob {
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub mode: DeleteMode,
    rx: Receiver<Vec<DeleteOutcome>>,
}

impl DeleteJob {
    pub fn start(targets: Vec<DeleteTarget>, mode: DeleteMode, ctx: egui::Context) -> Self {
        let total = targets.len();
        let done = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel();
        let d = done.clone();
        std::thread::spawn(move || {
            let mut outcomes = Vec::with_capacity(targets.len());
            for t in targets {
                let (result, kept) = if t.contents_only {
                    delete_contents(&t.path, mode)
                } else {
                    (delete_one(&t.path, mode), Vec::new())
                };
                outcomes.push(DeleteOutcome { target: t, result, kept_children: kept });
                d.fetch_add(1, Ordering::Relaxed);
                ctx.request_repaint();
            }
            let _ = tx.send(outcomes);
            ctx.request_repaint();
        });
        Self { total, done, mode, rx }
    }

    pub fn progress(&self) -> usize {
        self.done.load(Ordering::Relaxed)
    }

    pub fn poll(&self) -> Option<Vec<DeleteOutcome>> {
        self.rx.try_recv().ok()
    }
}

fn delete_one(path: &Path, mode: DeleteMode) -> Result<(), String> {
    if let Some(reason) = protection_reason(path) {
        return Err(format!("protected: {reason}"));
    }
    match mode {
        DeleteMode::RecycleBin => trash::delete(path).map_err(|e| e.to_string()),
        DeleteMode::Permanent => {
            let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                std::fs::remove_dir_all(path).map_err(|e| e.to_string())
            } else {
                std::fs::remove_file(path).map_err(|e| e.to_string())
            }
        }
    }
}

/// Delete everything inside a folder. Returns the names of children that failed (in use etc).
fn delete_contents(path: &Path, mode: DeleteMode) -> (Result<(), String>, Vec<String>) {
    if protection_reason(path).is_some() {
        return (Err("protected folder".into()), Vec::new());
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => return (Err(e.to_string()), Vec::new()),
    };
    let mut kept = Vec::new();
    let mut last_err = None;
    for entry in entries.flatten() {
        let p = entry.path();
        let r = match mode {
            DeleteMode::RecycleBin => trash::delete(&p).map_err(|e| e.to_string()),
            DeleteMode::Permanent => match entry.file_type() {
                Ok(t) if t.is_dir() => std::fs::remove_dir_all(&p).map_err(|e| e.to_string()),
                _ => std::fs::remove_file(&p).map_err(|e| e.to_string()),
            },
        };
        if let Err(e) = r {
            kept.push(entry.file_name().to_string_lossy().into_owned());
            last_err = Some(e);
        }
    }
    match last_err {
        Some(e) if !kept.is_empty() => (Err(format!("{} item(s) could not be removed (in use?): {e}", kept.len())), kept),
        _ => (Ok(()), kept),
    }
}

/// Run an uninstaller command line taken from the registry.
pub fn run_uninstaller(command_line: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .raw_arg(format!("/C \"{command_line}\""))
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = command_line;
        Err("Uninstalling is only supported on Windows".into())
    }
}

/// Open the system "Apps & features" settings page.
pub fn open_apps_settings() -> Result<(), String> {
    #[cfg(windows)]
    {
        open::that_detached("ms-settings:appsfeatures").map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Err("Not supported on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_delete_and_empty_folder() {
        let dir = std::env::temp_dir().join(format!("pcscanner_del_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("keep/inner")).unwrap();
        std::fs::write(dir.join("keep/a.txt"), b"a").unwrap();
        std::fs::write(dir.join("keep/inner/b.txt"), b"b").unwrap();
        std::fs::write(dir.join("gone.txt"), b"x").unwrap();

        assert!(delete_one(&dir.join("gone.txt"), DeleteMode::Permanent).is_ok());
        assert!(!dir.join("gone.txt").exists());

        let (r, kept) = delete_contents(&dir.join("keep"), DeleteMode::Permanent);
        assert!(r.is_ok());
        assert!(kept.is_empty());
        assert!(dir.join("keep").exists());
        assert_eq!(std::fs::read_dir(dir.join("keep")).unwrap().count(), 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn protects_roots() {
        #[cfg(windows)]
        {
            assert!(protection_reason(Path::new("C:\\")).is_some());
            assert!(protection_reason(Path::new("C:\\Windows")).is_some());
            assert!(protection_reason(Path::new("C:\\Windows\\System32\\drivers")).is_some());
            assert!(protection_reason(Path::new("C:\\pagefile.sys")).is_some());
            assert!(protection_reason(Path::new("C:\\Users\\x\\Downloads\\a.zip")).is_none());
        }
        #[cfg(unix)]
        {
            assert!(protection_reason(Path::new("/")).is_some());
            assert!(protection_reason(Path::new("/usr")).is_some());
            assert!(protection_reason(Path::new("/tmp/a.zip")).is_none());
        }
    }
}
