//! Background, parallel, cancellable file system scanner.

mod walker;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::analysis::Analysis;
use crate::analysis::cleanup::{self, CleanupItem};
use crate::model::FileTree;

/// Live counters shared between the scan threads and the UI.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub cancel: AtomicBool,
    pub current: Mutex<String>,
    /// Bytes used on the drives being scanned (for an approximate percentage).
    pub expected_bytes: AtomicU64,
}

impl Progress {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct ScanError {
    pub path: PathBuf,
    pub message: String,
}

pub struct ScanResult {
    pub tree: FileTree,
    pub analysis: Analysis,
    pub cleanup: Vec<CleanupItem>,
    pub errors: Vec<ScanError>,
    pub elapsed: Duration,
    pub cancelled: bool,
    pub roots: Vec<PathBuf>,
}

pub struct ScanHandle {
    pub progress: Arc<Progress>,
    pub started: Instant,
    rx: Receiver<ScanResult>,
}

impl ScanHandle {
    /// Start scanning the given roots on a background thread.
    pub fn start(roots: Vec<PathBuf>, expected_bytes: u64, ctx: egui::Context) -> Self {
        let progress = Arc::new(Progress::default());
        progress.expected_bytes.store(expected_bytes, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        let p = progress.clone();
        let r = roots;
        std::thread::Builder::new()
            .name("scanner".into())
            .spawn(move || {
                let started = Instant::now();
                let errors = Mutex::new(Vec::new());
                let temp_roots: Vec<_> = {
                    use rayon::prelude::*;
                    r.par_iter().map(|root| walker::scan_root(root, &p, &errors)).collect()
                };
                let mut tree = FileTree::default();
                if let Ok(mut cur) = p.current.lock() {
                    *cur = "Building index…".to_owned();
                }
                for t in temp_roots {
                    tree.add_root(t);
                }
                if let Ok(mut cur) = p.current.lock() {
                    *cur = "Analysing…".to_owned();
                }
                let analysis = Analysis::build(&tree);
                let cleanup = cleanup::suggestions(&tree);
                let result = ScanResult {
                    tree,
                    analysis,
                    cleanup,
                    errors: errors.into_inner().unwrap_or_default(),
                    elapsed: started.elapsed(),
                    cancelled: p.cancelled(),
                    roots: r,
                };
                let _ = tx.send(result);
                ctx.request_repaint();
            })
            .expect("failed to spawn scanner thread");
        Self { progress, started: Instant::now(), rx }
    }

    pub fn cancel(&self) {
        self.progress.cancel.store(true, Ordering::Relaxed);
    }

    /// Returns the result once the scan has finished.
    pub fn poll(&self) -> Option<ScanResult> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_a_directory() {
        let dir = std::env::temp_dir().join(format!("pcscanner_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("inner/deeper")).unwrap();
        fs::write(dir.join("a.txt"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("inner/b.mp4"), vec![0u8; 300]).unwrap();
        fs::write(dir.join("inner/deeper/c.zip"), vec![0u8; 50]).unwrap();

        let progress = Progress::default();
        let errors = Mutex::new(Vec::new());
        let temp = walker::scan_root(&dir, &progress, &errors);
        let mut tree = FileTree::default();
        tree.add_root(temp);
        let root = tree.roots[0];
        assert_eq!(tree.get(root).size, 450);
        assert_eq!(tree.get(root).files, 3);
        let inner = tree.find_path(&dir.join("inner")).unwrap();
        assert_eq!(tree.get(inner).size, 350);
        assert_eq!(progress.files.load(Ordering::Relaxed), 3);
        fs::remove_dir_all(&dir).unwrap();
    }
}
