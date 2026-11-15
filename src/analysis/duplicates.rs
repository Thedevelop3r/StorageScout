//! Duplicate file detection: size -> partial hash -> full hash.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};

use rayon::prelude::*;
use xxhash_rust::xxh3::Xxh3;

use crate::model::{FileTree, NodeId};

const PARTIAL: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct DupGroup {
    pub size: u64,
    pub ids: Vec<NodeId>,
}

impl DupGroup {
    /// Bytes that could be reclaimed by keeping only one copy.
    pub fn wasted(&self) -> u64 {
        self.size * (self.ids.len() as u64).saturating_sub(1)
    }
}

#[derive(Default)]
pub struct DupProgress {
    /// 0 = grouping, 1 = partial hashing, 2 = full hashing, 3 = done
    pub stage: AtomicU8,
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

pub struct DupJob {
    pub progress: Arc<DupProgress>,
    rx: Receiver<Vec<DupGroup>>,
}

impl DupJob {
    /// Collect candidate files from the tree and start hashing in the background.
    pub fn start(
        tree: &FileTree,
        files_by_size: &[NodeId],
        min_size: u64,
        ctx: egui::Context,
    ) -> Self {
        // files_by_size is sorted desc; group equal sizes.
        let mut candidates: Vec<(NodeId, PathBuf, u64)> = Vec::new();
        let mut i = 0;
        let ids: Vec<NodeId> = files_by_size
            .iter()
            .copied()
            .filter(|&id| !tree.get(id).deleted && tree.get(id).size >= min_size.max(1))
            .collect();
        while i < ids.len() {
            let size = tree.get(ids[i]).size;
            let mut j = i + 1;
            while j < ids.len() && tree.get(ids[j]).size == size {
                j += 1;
            }
            if j - i >= 2 {
                for &id in &ids[i..j] {
                    candidates.push((id, tree.path(id), size));
                }
            }
            i = j;
        }

        let progress = Arc::new(DupProgress::default());
        let (tx, rx) = mpsc::channel();
        let p = progress.clone();
        std::thread::spawn(move || {
            let groups = find_duplicates(candidates, &p);
            p.stage.store(3, Ordering::Relaxed);
            let _ = tx.send(groups);
            ctx.request_repaint();
        });
        Self { progress, rx }
    }

    pub fn cancel(&self) {
        self.progress.cancel.store(true, Ordering::Relaxed);
    }

    pub fn poll(&self) -> Option<Vec<DupGroup>> {
        self.rx.try_recv().ok()
    }
}

fn find_duplicates(candidates: Vec<(NodeId, PathBuf, u64)>, p: &DupProgress) -> Vec<DupGroup> {
    // Stage 1: partial hash (first + last 64 KiB).
    p.stage.store(1, Ordering::Relaxed);
    p.total.store(candidates.len() as u64, Ordering::Relaxed);
    p.done.store(0, Ordering::Relaxed);
    let partial: Vec<(NodeId, PathBuf, u64, u64)> = candidates
        .into_par_iter()
        .filter_map(|(id, path, size)| {
            if p.cancel.load(Ordering::Relaxed) {
                return None;
            }
            let h = hash_partial(&path, size).ok();
            p.done.fetch_add(1, Ordering::Relaxed);
            h.map(|h| (id, path, size, h))
        })
        .collect();

    let mut by_key: HashMap<(u64, u64), Vec<(NodeId, PathBuf)>> = HashMap::new();
    for (id, path, size, h) in partial {
        by_key.entry((size, h)).or_default().push((id, path));
    }
    type Group = ((u64, u64), Vec<(NodeId, PathBuf)>);
    let survivors: Vec<Group> = by_key.into_iter().filter(|(_, v)| v.len() >= 2).collect();

    // Stage 2: full hash for files bigger than what the partial hash already covered.
    p.stage.store(2, Ordering::Relaxed);
    let total_full: u64 = survivors
        .iter()
        .filter(|((size, _), _)| *size > (PARTIAL * 2) as u64)
        .map(|((size, _), v)| size * v.len() as u64)
        .sum();
    p.total.store(total_full.max(1), Ordering::Relaxed);
    p.done.store(0, Ordering::Relaxed);

    let mut groups: Vec<DupGroup> = survivors
        .into_par_iter()
        .flat_map_iter(|((size, _), files)| {
            if size <= (PARTIAL * 2) as u64 {
                // partial hash already covered the full file
                return vec![DupGroup {
                    size,
                    ids: files.into_iter().map(|f| f.0).collect(),
                }];
            }
            let mut by_full: HashMap<u64, Vec<NodeId>> = HashMap::new();
            for (id, path) in files {
                if p.cancel.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(h) = hash_full(&path, p) {
                    by_full.entry(h).or_default().push(id);
                }
            }
            by_full
                .into_values()
                .filter(|v| v.len() >= 2)
                .map(|ids| DupGroup { size, ids })
                .collect()
        })
        .collect();
    groups.sort_unstable_by_key(|g| std::cmp::Reverse(g.wasted()));
    groups
}

fn hash_partial(path: &Path, size: u64) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    let mut h = Xxh3::new();
    let mut buf = vec![0u8; PARTIAL];
    let n = read_up_to(&mut f, &mut buf)?;
    h.update(&buf[..n]);
    if size > (PARTIAL * 2) as u64 {
        f.seek(SeekFrom::End(-(PARTIAL as i64)))?;
        let n = read_up_to(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    } else if size > PARTIAL as u64 {
        let n = read_up_to(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(h.digest())
}

fn hash_full(path: &Path, p: &DupProgress) -> std::io::Result<u64> {
    let mut f = File::open(path)?;
    let mut h = Xxh3::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        if p.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("cancelled"));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        p.done.fetch_add(n as u64, Ordering::Relaxed);
    }
    Ok(h.digest())
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = f.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_duplicates() {
        let dir = std::env::temp_dir().join(format!("pcscanner_dup_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let big: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = big.clone();
        other[150_000] ^= 0xFF; // same size, same head/tail, different middle
        fs::write(dir.join("a.bin"), &big).unwrap();
        fs::write(dir.join("b.bin"), &big).unwrap();
        fs::write(dir.join("c.bin"), &other).unwrap();
        fs::write(dir.join("d.txt"), b"hello").unwrap();
        fs::write(dir.join("e.txt"), b"hello").unwrap();

        let cands: Vec<(NodeId, PathBuf, u64)> = vec![
            (0, dir.join("a.bin"), big.len() as u64),
            (1, dir.join("b.bin"), big.len() as u64),
            (2, dir.join("c.bin"), big.len() as u64),
            (3, dir.join("d.txt"), 5),
            (4, dir.join("e.txt"), 5),
        ];
        let p = DupProgress::default();
        let groups = find_duplicates(cands, &p);
        assert_eq!(groups.len(), 2);
        let mut big_group = groups.iter().find(|g| g.size > 5).unwrap().ids.clone();
        big_group.sort();
        assert_eq!(big_group, vec![0, 1]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
