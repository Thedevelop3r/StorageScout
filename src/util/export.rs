//! CSV / JSON export of scan results.

use std::path::Path;

use serde::Serialize;

use crate::model::Category;
use crate::system::programs::Program;
use crate::ui::ScanData;

pub fn export_files_csv(data: &ScanData, path: &Path, limit: usize) -> Result<usize, String> {
    let mut w = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
    w.write_record(["path", "size_bytes", "size", "category", "modified"]).map_err(|e| e.to_string())?;
    let tree = &data.tree;
    let mut n = 0;
    for &id in data.analysis.files_by_size.iter().filter(|&&id| !tree.get(id).deleted).take(limit) {
        let node = tree.get(id);
        w.write_record([
            tree.path(id).to_string_lossy().as_ref(),
            &node.size.to_string(),
            &super::format::size(node.size),
            node.category.label(),
            &super::format::date(node.modified),
        ])
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(n)
}

pub fn export_folders_csv(data: &ScanData, path: &Path, limit: usize) -> Result<usize, String> {
    let mut w = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
    w.write_record(["path", "size_bytes", "size", "files", "modified"]).map_err(|e| e.to_string())?;
    let tree = &data.tree;
    let mut n = 0;
    for &id in data.analysis.dirs_by_size.iter().filter(|&&id| !tree.get(id).deleted).take(limit) {
        let node = tree.get(id);
        w.write_record([
            tree.path(id).to_string_lossy().as_ref(),
            &node.size.to_string(),
            &super::format::size(node.size),
            &node.files.to_string(),
            &super::format::date(node.modified),
        ])
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(n)
}

pub fn export_programs_csv(programs: &[Program], path: &Path) -> Result<usize, String> {
    let mut w = csv::Writer::from_path(path).map_err(|e| e.to_string())?;
    w.write_record(["name", "version", "publisher", "install_date", "estimated_size_bytes", "source", "location", "uninstall"])
        .map_err(|e| e.to_string())?;
    for p in programs {
        w.write_record([
            p.name.as_str(),
            &p.version,
            &p.publisher,
            &p.install_date_display(),
            &p.estimated_size.to_string(),
            p.source.label(),
            &p.install_location.as_ref().map(|l| l.to_string_lossy().into_owned()).unwrap_or_default(),
            p.uninstall_string.as_deref().unwrap_or_default(),
        ])
        .map_err(|e| e.to_string())?;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(programs.len())
}

#[derive(Serialize)]
struct Summary {
    generated: String,
    roots: Vec<String>,
    total_size_bytes: u64,
    total_files: u64,
    total_folders: u64,
    scan_seconds: f64,
    inaccessible: usize,
    categories: Vec<CatOut>,
    top_extensions: Vec<ExtOut>,
    largest_files: Vec<Item>,
    largest_folders: Vec<Item>,
    cleanup: Vec<CleanOut>,
}

#[derive(Serialize)]
struct CatOut {
    category: Category,
    label: &'static str,
    size_bytes: u64,
    files: u64,
}

#[derive(Serialize)]
struct ExtOut {
    extension: String,
    size_bytes: u64,
    files: u64,
}

#[derive(Serialize)]
struct Item {
    path: String,
    size_bytes: u64,
}

#[derive(Serialize)]
struct CleanOut {
    group: &'static str,
    title: String,
    size_bytes: u64,
    locations: usize,
}

pub fn export_summary_json(data: &ScanData, path: &Path) -> Result<(), String> {
    let tree = &data.tree;
    let a = &data.analysis;
    let items = |ids: &[crate::model::NodeId]| -> Vec<Item> {
        ids.iter()
            .filter(|&&id| !tree.get(id).deleted)
            .take(100)
            .map(|&id| Item { path: tree.path(id).to_string_lossy().into_owned(), size_bytes: tree.get(id).size })
            .collect()
    };
    let summary = Summary {
        generated: chrono::Local::now().to_rfc3339(),
        roots: data.roots.iter().map(|r| r.to_string_lossy().into_owned()).collect(),
        total_size_bytes: a.total_size,
        total_files: a.total_files,
        total_folders: a.total_dirs,
        scan_seconds: data.elapsed_secs,
        inaccessible: data.errors.len(),
        categories: Category::ALL
            .iter()
            .map(|&c| CatOut { category: c, label: c.label(), size_bytes: a.categories[c.index()].size, files: a.categories[c.index()].count })
            .collect(),
        top_extensions: a.extensions.iter().take(50).map(|e| ExtOut { extension: e.ext.clone(), size_bytes: e.size, files: e.count }).collect(),
        largest_files: items(&a.files_by_size),
        largest_folders: items(&a.dirs_by_size),
        cleanup: data
            .cleanup
            .iter()
            .map(|c| CleanOut { group: c.group, title: c.title.clone(), size_bytes: c.size, locations: c.targets.len() })
            .collect(),
    };
    let json = serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
