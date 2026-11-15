//! Filterable lists: largest files, largest folders, old files, empty folders.

use egui::{RichText, Ui};

use super::{Ctx, TableOptions, UiAction, node_table};
use crate::model::{Category, NodeId};
use crate::util::format;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ListKind {
    LargestFiles,
    LargestFolders,
    OldFiles,
    EmptyFolders,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Size,
    Oldest,
    Newest,
    Name,
}

pub struct FileListState {
    pub kind: ListKind,
    pub search: String,
    pub category: Option<Category>,
    pub min_size_mb: f64,
    pub old_days: u32,
    pub sort: SortBy,
    /// Folders: hide folders that are inside another listed folder.
    pub top_level_only: bool,
    pub limit: usize,
    cache: Vec<NodeId>,
    cache_key: Option<String>,
}

impl FileListState {
    pub fn new(kind: ListKind) -> Self {
        Self {
            kind,
            search: String::new(),
            category: None,
            min_size_mb: match kind {
                ListKind::OldFiles => 1.0,
                _ => 0.0,
            },
            old_days: 365,
            sort: SortBy::Size,
            top_level_only: false,
            limit: 20_000,
            cache: Vec::new(),
            cache_key: None,
        }
    }

    fn key(&self, generation: u64) -> String {
        format!(
            "{generation}|{}|{:?}|{}|{}|{}|{}|{}",
            self.search,
            self.category,
            self.min_size_mb,
            self.old_days,
            self.sort as u8,
            self.top_level_only,
            self.limit
        )
    }
}

impl SortBy {
    fn label(self) -> &'static str {
        match self {
            SortBy::Size => "Largest first",
            SortBy::Oldest => "Oldest first",
            SortBy::Newest => "Newest first",
            SortBy::Name => "Name",
        }
    }
}

pub fn show(ui: &mut Ui, st: &mut FileListState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    let a = &data.analysis;

    let (title, help) = match st.kind {
        ListKind::LargestFiles => ("Largest files", "Every file on the scanned drives, biggest first."),
        ListKind::LargestFolders => ("Largest folders", "Folders by total size including everything inside."),
        ListKind::OldFiles => ("Old files", "Files that have not been modified for a long time — good archive or cleanup candidates."),
        ListKind::EmptyFolders => ("Empty folders", "Folders that contain no files at all (possibly empty sub-folders)."),
    };
    ui.heading(title);
    ui.label(RichText::new(help).weak());

    ui.horizontal_wrapped(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Name or path contains…").desired_width(200.0));
        if matches!(st.kind, ListKind::LargestFiles | ListKind::OldFiles) {
            egui::ComboBox::from_id_salt(("cat", st.kind as u8))
                .selected_text(st.category.map(|c| format!("{} {}", c.icon(), c.label())).unwrap_or_else(|| "All types".into()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut st.category, None, "All types");
                    for c in Category::ALL {
                        ui.selectable_value(&mut st.category, Some(c), format!("{} {}", c.icon(), c.label()));
                    }
                });
        }
        if st.kind != ListKind::EmptyFolders {
            ui.label("Min size (MB)");
            ui.add(egui::DragValue::new(&mut st.min_size_mb).range(0.0..=1_000_000.0).speed(1.0));
        }
        if st.kind == ListKind::OldFiles {
            ui.label("Not modified for");
            egui::ComboBox::from_id_salt("old_days")
                .selected_text(days_label(st.old_days))
                .show_ui(ui, |ui| {
                    for d in [90, 180, 365, 730, 1095, 1825, 3650] {
                        ui.selectable_value(&mut st.old_days, d, days_label(d));
                    }
                });
        }
        if st.kind == ListKind::LargestFolders {
            ui.checkbox(&mut st.top_level_only, "Hide nested folders")
                .on_hover_text("Only show the outermost of nested large folders");
        }
        if st.kind != ListKind::EmptyFolders {
            egui::ComboBox::from_id_salt(("sort", st.kind as u8))
                .selected_text(st.sort.label())
                .show_ui(ui, |ui| {
                    for s in [SortBy::Size, SortBy::Oldest, SortBy::Newest, SortBy::Name] {
                        ui.selectable_value(&mut st.sort, s, s.label());
                    }
                });
        }
    });

    // Rebuild the cached list when filters or data change.
    let key = st.key(data.generation);
    if st.cache_key.as_ref() != Some(&key) {
        let min = (st.min_size_mb * 1024.0 * 1024.0) as u64;
        let search = st.search.to_lowercase();
        let now = chrono::Local::now().timestamp();
        let cutoff = now - st.old_days as i64 * 86_400;
        let source: &[NodeId] = match st.kind {
            ListKind::LargestFiles | ListKind::OldFiles => &a.files_by_size,
            ListKind::LargestFolders => &a.dirs_by_size,
            ListKind::EmptyFolders => &a.empty_dirs,
        };
        let mut out: Vec<NodeId> = Vec::new();
        for &id in source {
            let n = tree.get(id);
            if n.deleted {
                continue;
            }
            if n.size < min && st.kind != ListKind::EmptyFolders {
                break; // sorted by size
            }
            if st.kind == ListKind::LargestFolders && n.parent == crate::model::tree::NO_PARENT {
                continue;
            }
            if let Some(c) = st.category
                && n.category != c
            {
                continue;
            }
            if st.kind == ListKind::OldFiles && (n.modified == 0 || n.modified > cutoff) {
                continue;
            }
            if !search.is_empty()
                && !n.name.to_lowercase().contains(&search)
                && !tree.path(id).to_string_lossy().to_lowercase().contains(&search)
            {
                continue;
            }
            if st.top_level_only && out.iter().rev().take(2000).any(|&o| tree.is_within(id, o)) {
                continue;
            }
            out.push(id);
            if out.len() >= st.limit {
                break;
            }
        }
        match st.sort {
            SortBy::Size => {}
            SortBy::Oldest => out.sort_by_key(|&id| tree.get(id).modified),
            SortBy::Newest => out.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).modified)),
            SortBy::Name => out.sort_by_cached_key(|&id| tree.get(id).name.to_lowercase()),
        }
        st.cache = out;
        st.cache_key = Some(key);
    }

    let total: u64 = st.cache.iter().map(|&id| tree.get(id).size).sum();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} items · {}", format::count(st.cache.len() as u64), format::size(total))).strong());
        if st.cache.len() >= st.limit {
            ui.label(RichText::new(format!("(showing the first {})", format::count(st.limit as u64))).weak());
        }
        if !st.cache.is_empty() {
            if ui.button("☑ Select all listed").clicked() {
                ctx.actions.push(UiAction::Select(st.cache.clone()));
            }
            if ui.button("☐ Unselect listed").clicked() {
                ctx.actions.push(UiAction::Unselect(st.cache.clone()));
            }
        }
    });
    ui.separator();

    let rows = std::mem::take(&mut st.cache);
    let out = node_table(
        ui,
        &format!("list_{:?}", st.kind),
        ctx,
        &rows,
        TableOptions {
            show_location: true,
            show_files: matches!(st.kind, ListKind::LargestFolders),
            show_age: true,
            bar_total: 0,
        },
    );
    if let Some(id) = out.activated {
        if tree.get(id).is_dir {
            ctx.actions.push(UiAction::ShowInExplorer(id));
        } else {
            ctx.actions.push(UiAction::Open(tree.path(id)));
        }
    }
    st.cache = rows;
}

fn days_label(d: u32) -> String {
    if d >= 365 && d.is_multiple_of(365) {
        let y = d / 365;
        if y == 1 { "1 year".into() } else { format!("{y} years") }
    } else {
        format!("{d} days")
    }
}
