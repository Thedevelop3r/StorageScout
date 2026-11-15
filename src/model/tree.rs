//! Flat arena representation of a scanned file system.
//!
//! Every node lives in a single `Vec<Node>`. The children of a directory are stored
//! contiguously (`children_start .. children_start + children_len`) and sorted by size,
//! largest first. Roots store their full path as their name.

use std::path::{Path, PathBuf};

use super::category::Category;

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;

#[derive(Debug, Clone)]
pub struct Node {
    pub name: Box<str>,
    pub parent: NodeId,
    pub children_start: u32,
    pub children_len: u32,
    /// Logical size in bytes (recursive for directories).
    pub size: u64,
    /// Number of files inside (recursive). 1 for a file.
    pub files: u32,
    /// Last modification time (unix seconds), 0 if unknown.
    pub modified: i64,
    pub is_dir: bool,
    pub category: Category,
    pub deleted: bool,
    /// Directory could not be (fully) read.
    pub error: bool,
}

impl Node {
    pub fn children(&self) -> std::ops::Range<NodeId> {
        self.children_start..self.children_start + self.children_len
    }
}

#[derive(Debug, Default)]
pub struct FileTree {
    pub nodes: Vec<Node>,
    pub roots: Vec<NodeId>,
}

impl FileTree {
    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Children of `id` that have not been deleted.
    pub fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.get(id)
            .children()
            .filter(move |&c| !self.get(c).deleted)
    }

    pub fn total_size(&self) -> u64 {
        self.roots.iter().map(|&r| self.get(r).size).sum()
    }

    /// Rebuild the full path of a node.
    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = id;
        loop {
            let n = self.get(cur);
            parts.push(&n.name);
            if n.parent == NO_PARENT {
                break;
            }
            cur = n.parent;
        }
        let mut path = PathBuf::from(parts.pop().unwrap_or_default());
        for p in parts.into_iter().rev() {
            path.push(p);
        }
        path
    }

    /// Chain of ancestors from the root down to `id` (inclusive).
    pub fn ancestry(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = vec![id];
        let mut cur = id;
        while self.get(cur).parent != NO_PARENT {
            cur = self.get(cur).parent;
            chain.push(cur);
        }
        chain.reverse();
        chain
    }

    /// The root that contains `id`.
    pub fn root_of(&self, id: NodeId) -> NodeId {
        let mut cur = id;
        while self.get(cur).parent != NO_PARENT {
            cur = self.get(cur).parent;
        }
        cur
    }

    /// True if `id` or any ancestor was deleted.
    pub fn is_gone(&self, id: NodeId) -> bool {
        let mut cur = id;
        loop {
            let n = self.get(cur);
            if n.deleted {
                return true;
            }
            if n.parent == NO_PARENT {
                return false;
            }
            cur = n.parent;
        }
    }

    /// True if `ancestor` is `id` or one of its ancestors.
    pub fn is_within(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut cur = id;
        loop {
            if cur == ancestor {
                return true;
            }
            let p = self.get(cur).parent;
            if p == NO_PARENT {
                return false;
            }
            cur = p;
        }
    }

    /// Find a node by absolute path (case-insensitive on Windows).
    pub fn find_path(&self, path: &Path) -> Option<NodeId> {
        let target = normalize(&path.to_string_lossy());
        for &root in &self.roots {
            let root_name = normalize(&self.get(root).name);
            let root_trim = root_name.trim_end_matches(['\\', '/']);
            if target == root_name || target == root_trim {
                return Some(root);
            }
            let rest = if let Some(r) = target.strip_prefix(&root_name) {
                r
            } else if let Some(r) = target.strip_prefix(root_trim) {
                match r.strip_prefix(['\\', '/']) {
                    Some(r) => r,
                    None => continue,
                }
            } else {
                continue;
            };
            let mut cur = root;
            let mut found = true;
            for comp in rest.split(['\\', '/']).filter(|c| !c.is_empty()) {
                match self.get(cur).children().find(|&c| {
                    let n = self.get(c);
                    !n.deleted && names_equal(&n.name, comp)
                }) {
                    Some(c) => cur = c,
                    None => {
                        found = false;
                        break;
                    }
                }
            }
            if found {
                return Some(cur);
            }
        }
        None
    }

    /// Mark a node (and its subtree) deleted and subtract its size from all ancestors.
    pub fn mark_deleted(&mut self, id: NodeId) {
        if self.get(id).deleted {
            return;
        }
        let size = self.get(id).size;
        let files = self.get(id).files;
        // mark subtree
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            let node = &mut self.nodes[n as usize];
            node.deleted = true;
            stack.extend(node.children());
        }
        // update ancestors
        let mut cur = self.get(id).parent;
        while cur != NO_PARENT {
            let node = &mut self.nodes[cur as usize];
            node.size = node.size.saturating_sub(size);
            node.files = node.files.saturating_sub(files);
            cur = node.parent;
        }
    }
}

fn normalize(s: &str) -> String {
    if cfg!(windows) {
        s.replace('/', "\\").to_lowercase()
    } else {
        s.to_owned()
    }
}

fn names_equal(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

// ---------------------------------------------------------------------------
// Building

/// Intermediate recursive directory produced by the scanner.
#[derive(Debug, Default)]
pub struct TempDir {
    pub name: Box<str>,
    pub modified: i64,
    pub size: u64,
    pub files_count: u32,
    pub files: Vec<TempFile>,
    pub dirs: Vec<TempDir>,
    pub error: bool,
}

#[derive(Debug)]
pub struct TempFile {
    pub name: Box<str>,
    pub size: u64,
    pub modified: i64,
}

impl TempDir {
    /// Compute recursive sizes (call once, bottom up).
    pub fn finalize(&mut self) {
        let mut size: u64 = self.files.iter().map(|f| f.size).sum();
        let mut count = self.files.len() as u32;
        for d in &self.dirs {
            size += d.size;
            count += d.files_count;
        }
        self.size = size;
        self.files_count = count;
    }
}

impl FileTree {
    /// Append a scanned root (consumes the temporary tree).
    pub fn add_root(&mut self, root: TempDir) {
        use std::collections::VecDeque;
        let root_id = self.nodes.len() as NodeId;
        self.nodes.push(dir_node(&root, NO_PARENT));
        self.roots.push(root_id);

        let mut queue: VecDeque<(TempDir, NodeId)> = VecDeque::new();
        queue.push_back((root, root_id));

        while let Some((mut dir, id)) = queue.pop_front() {
            // gather children sorted by size desc
            enum Child {
                F(TempFile),
                D(TempDir),
            }
            let mut kids: Vec<Child> = Vec::with_capacity(dir.files.len() + dir.dirs.len());
            kids.extend(std::mem::take(&mut dir.dirs).into_iter().map(Child::D));
            kids.extend(std::mem::take(&mut dir.files).into_iter().map(Child::F));
            kids.sort_by_key(|c| {
                std::cmp::Reverse(match c {
                    Child::F(f) => f.size,
                    Child::D(d) => d.size,
                })
            });

            let start = self.nodes.len() as u32;
            self.nodes[id as usize].children_start = start;
            self.nodes[id as usize].children_len = kids.len() as u32;
            for kid in kids {
                let child_id = self.nodes.len() as NodeId;
                match kid {
                    Child::F(f) => {
                        let category = Category::from_name(&f.name);
                        self.nodes.push(Node {
                            name: f.name,
                            parent: id,
                            children_start: 0,
                            children_len: 0,
                            size: f.size,
                            files: 1,
                            modified: f.modified,
                            is_dir: false,
                            category,
                            deleted: false,
                            error: false,
                        });
                    }
                    Child::D(d) => {
                        self.nodes.push(dir_node(&d, id));
                        queue.push_back((d, child_id));
                    }
                }
            }
        }
    }
}

fn dir_node(d: &TempDir, parent: NodeId) -> Node {
    Node {
        name: d.name.clone(),
        parent,
        children_start: 0,
        children_len: 0,
        size: d.size,
        files: d.files_count,
        modified: d.modified,
        is_dir: true,
        category: Category::Other,
        deleted: false,
        error: d.error,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample_tree() -> FileTree {
        let root_name = if cfg!(windows) { "C:\\data" } else { "/data" };
        let mut sub = TempDir {
            name: "sub".into(),
            files: vec![
                TempFile {
                    name: "big.mkv".into(),
                    size: 1000,
                    modified: 1,
                },
                TempFile {
                    name: "small.txt".into(),
                    size: 10,
                    modified: 2,
                },
            ],
            ..Default::default()
        };
        sub.finalize();
        let mut root = TempDir {
            name: root_name.into(),
            files: vec![TempFile {
                name: "a.mp3".into(),
                size: 500,
                modified: 3,
            }],
            dirs: vec![sub],
            ..Default::default()
        };
        root.finalize();
        let mut t = FileTree::default();
        t.add_root(root);
        t
    }

    #[test]
    fn build_and_sizes() {
        let t = sample_tree();
        let r = t.roots[0];
        assert_eq!(t.get(r).size, 1510);
        assert_eq!(t.get(r).files, 3);
        let kids: Vec<_> = t.children(r).collect();
        assert_eq!(kids.len(), 2);
        // sorted: sub (1010) then a.mp3 (500)
        assert_eq!(&*t.get(kids[0]).name, "sub");
        assert_eq!(t.get(kids[0]).size, 1010);
    }

    #[test]
    fn paths_and_find() {
        let t = sample_tree();
        let r = t.roots[0];
        let sub = t.children(r).next().unwrap();
        let big = t.children(sub).next().unwrap();
        let p = t.path(big);
        assert!(p.ends_with("big.mkv"));
        assert_eq!(t.find_path(&p), Some(big));
        assert_eq!(t.find_path(&t.path(r)), Some(r));
        assert!(t.is_within(big, r));
    }

    #[test]
    fn delete_updates_ancestors() {
        let mut t = sample_tree();
        let r = t.roots[0];
        let sub = t.children(r).next().unwrap();
        let big = t.children(sub).next().unwrap();
        t.mark_deleted(big);
        assert_eq!(t.get(sub).size, 10);
        assert_eq!(t.get(r).size, 510);
        assert_eq!(t.get(r).files, 2);
        t.mark_deleted(sub);
        assert_eq!(t.get(r).size, 500);
        assert!(t.is_gone(big));
        assert_eq!(t.children(r).count(), 1);
    }
}
