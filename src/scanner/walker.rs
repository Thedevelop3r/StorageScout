//! Recursive parallel directory walker.

use std::fs::{self, Metadata};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;

use rayon::prelude::*;

use super::{Progress, ScanError};
use crate::model::tree::{TempDir, TempFile};

const MAX_ERRORS: usize = 20_000;

pub fn scan_root(root: &Path, progress: &Progress, errors: &Mutex<Vec<ScanError>>) -> TempDir {
    let modified = fs::metadata(root).map(|m| mtime(&m)).unwrap_or(0);
    let mut dir = scan_dir(
        root,
        root.to_string_lossy().into(),
        modified,
        progress,
        errors,
    );
    dir.finalize();
    dir
}

fn scan_dir(
    path: &Path,
    name: Box<str>,
    modified: i64,
    progress: &Progress,
    errors: &Mutex<Vec<ScanError>>,
) -> TempDir {
    let mut dir = TempDir {
        name,
        modified,
        ..Default::default()
    };
    if progress.cancelled() {
        return dir;
    }
    progress.dirs.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut cur) = progress.current.try_lock() {
        cur.clear();
        cur.push_str(&path.to_string_lossy());
    }

    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) => {
            dir.error = true;
            record_error(progress, errors, path, &e);
            return dir;
        }
    };

    let mut subdirs: Vec<(std::path::PathBuf, Box<str>, i64)> = Vec::new();
    let mut bytes_here = 0u64;
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                dir.error = true;
                record_error(progress, errors, path, &e);
                continue;
            }
        };
        let name: Box<str> = entry.file_name().to_string_lossy().into();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(e) => {
                record_error(progress, errors, &entry.path(), &e);
                continue;
            }
        };
        // Never follow symlinks / junctions: avoids loops and double counting.
        if file_type.is_symlink() {
            continue;
        }
        let meta = entry.metadata().ok();
        let modified = meta.as_ref().map(mtime).unwrap_or(0);
        if file_type.is_dir() {
            subdirs.push((entry.path(), name, modified));
        } else {
            let size = meta.as_ref().map(file_size).unwrap_or(0);
            bytes_here += size;
            dir.files.push(TempFile {
                name,
                size,
                modified,
            });
        }
    }
    progress
        .files
        .fetch_add(dir.files.len() as u64, Ordering::Relaxed);
    progress.bytes.fetch_add(bytes_here, Ordering::Relaxed);

    dir.dirs = subdirs
        .into_par_iter()
        .map(|(p, n, m)| {
            let mut d = scan_dir(&p, n, m, progress, errors);
            d.finalize();
            d
        })
        .collect();
    dir.files.shrink_to_fit();
    dir
}

fn record_error(
    progress: &Progress,
    errors: &Mutex<Vec<ScanError>>,
    path: &Path,
    e: &std::io::Error,
) {
    progress.errors.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut errs) = errors.lock()
        && errs.len() < MAX_ERRORS
    {
        errs.push(ScanError {
            path: path.to_path_buf(),
            message: e.to_string(),
        });
    }
}

fn mtime(m: &Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Size of a file as it counts against local disk usage.
fn file_size(m: &Metadata) -> u64 {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
        const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
        const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
        // Cloud-only placeholders (OneDrive etc.) take no local space.
        if m.file_attributes()
            & (FILE_ATTRIBUTE_OFFLINE
                | FILE_ATTRIBUTE_RECALL_ON_OPEN
                | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
            != 0
        {
            return 0;
        }
    }
    m.len()
}
