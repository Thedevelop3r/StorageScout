//! Aggregated statistics computed once after a scan (and refreshed after deletions).

pub mod cleanup;
pub mod duplicates;

use std::collections::HashMap;

use rayon::slice::ParallelSliceMut;

use crate::model::category::extension_of;
use crate::model::{Category, FileTree, NodeId};

#[derive(Debug, Clone, Copy, Default)]
pub struct CatStat {
    pub size: u64,
    pub count: u64,
}

#[derive(Debug, Clone)]
pub struct ExtStat {
    pub ext: String,
    pub size: u64,
    pub count: u64,
    pub category: Category,
    pub largest: NodeId,
}

#[derive(Default)]
pub struct Analysis {
    pub categories: [CatStat; Category::ALL.len()],
    pub extensions: Vec<ExtStat>,
    /// All files, largest first.
    pub files_by_size: Vec<NodeId>,
    /// All directories, largest first.
    pub dirs_by_size: Vec<NodeId>,
    /// Directories that contain no files at all (recursively).
    pub empty_dirs: Vec<NodeId>,
    /// Executables (.exe / .app bundles etc.).
    pub executables: Vec<NodeId>,
    pub total_size: u64,
    pub total_files: u64,
    pub total_dirs: u64,
}

impl Analysis {
    pub fn build(tree: &FileTree) -> Self {
        let mut a = Analysis::default();
        let mut ext_map: HashMap<String, ExtStat> = HashMap::new();
        for (i, n) in tree.nodes.iter().enumerate() {
            if n.deleted {
                continue;
            }
            let id = i as NodeId;
            if n.is_dir {
                a.total_dirs += 1;
                a.dirs_by_size.push(id);
                if n.files == 0 && n.parent != crate::model::tree::NO_PARENT && !n.error {
                    // only the top-most empty directory of an empty chain
                    let parent = tree.get(n.parent);
                    if parent.files != 0 {
                        a.empty_dirs.push(id);
                    }
                }
                continue;
            }
            a.total_files += 1;
            a.total_size += n.size;
            let c = &mut a.categories[n.category.index()];
            c.size += n.size;
            c.count += 1;
            a.files_by_size.push(id);

            let ext = extension_of(&n.name);
            if ext == "exe" {
                a.executables.push(id);
            }
            let e = ext_map.entry(ext).or_insert_with_key(|k| ExtStat {
                ext: k.clone(),
                size: 0,
                count: 0,
                category: n.category,
                largest: id,
            });
            e.size += n.size;
            e.count += 1;
            if n.size > tree.get(e.largest).size {
                e.largest = id;
            }
        }
        a.files_by_size.par_sort_unstable_by_key(|&id| std::cmp::Reverse(tree.get(id).size));
        a.dirs_by_size.par_sort_unstable_by_key(|&id| std::cmp::Reverse(tree.get(id).size));
        a.executables.sort_unstable_by_key(|&id| std::cmp::Reverse(tree.get(id).size));
        a.extensions = ext_map.into_values().collect();
        a.extensions.sort_unstable_by_key(|e| std::cmp::Reverse(e.size));
        a
    }

    pub fn category_total(&self) -> u64 {
        self.categories.iter().map(|c| c.size).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tree::tests::sample_tree;

    #[test]
    fn analysis_totals() {
        let t = sample_tree();
        let a = Analysis::build(&t);
        assert_eq!(a.total_files, 3);
        assert_eq!(a.total_size, 1510);
        assert_eq!(a.categories[Category::Video.index()].size, 1000);
        assert_eq!(a.categories[Category::Audio.index()].count, 1);
        assert_eq!(t.get(a.files_by_size[0]).size, 1000);
        assert_eq!(a.extensions[0].ext, "mkv");
    }
}
