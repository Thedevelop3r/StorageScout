//! Views and shared widgets.

pub mod categories;
pub mod cleanup;
pub mod duplicates;
pub mod errors;
pub mod explorer;
pub mod extensions;
pub mod files;
pub mod overview;
pub mod programs;
pub mod search;
pub mod treemap;

use std::collections::BTreeSet;
use std::path::PathBuf;

use egui::{Color32, Response, RichText, Sense, Ui, Vec2};
use egui_extras::{Column, TableBuilder};

use crate::analysis::Analysis;
use crate::analysis::cleanup::CleanupItem;
use crate::model::{FileTree, NodeId};
use crate::scanner::ScanError;
use crate::util::format;

/// Everything known about a finished scan.
pub struct ScanData {
    pub tree: FileTree,
    pub analysis: Analysis,
    pub cleanup: Vec<CleanupItem>,
    pub errors: Vec<ScanError>,
    pub elapsed_secs: f64,
    pub cancelled: bool,
    pub roots: Vec<PathBuf>,
    /// Bumped whenever the tree changes (deletions), so views can refresh caches.
    pub generation: u64,
}

/// What a view asks the app to do; processed after the frame.
#[derive(Debug, Clone)]
pub enum UiAction {
    Open(PathBuf),
    Reveal(PathBuf),
    CopyText(String),
    ToggleSelect(NodeId),
    Select(Vec<NodeId>),
    Unselect(Vec<NodeId>),
    /// Ask for confirmation, then delete these nodes (whole).
    Delete(Vec<NodeId>),
    /// Ask for confirmation, then delete the contents of these folders.
    DeleteContents(Vec<NodeId>),
    ShowInExplorer(NodeId),
    ShowInTreemap(NodeId),
    Notify(String),
}

/// Read-only context shared with every view.
pub struct Ctx<'a> {
    pub data: &'a ScanData,
    pub selection: &'a BTreeSet<NodeId>,
    pub actions: &'a mut Vec<UiAction>,
}

pub fn node_icon(tree: &FileTree, id: NodeId) -> &'static str {
    let n = tree.get(id);
    if n.is_dir {
        if n.parent == crate::model::tree::NO_PARENT { "💽" } else { "📁" }
    } else {
        n.category.icon()
    }
}

/// A thin horizontal bar showing `fraction` of the available width.
pub fn size_bar(ui: &mut Ui, fraction: f32, color: Color32, width: f32) -> Response {
    let height = ui.spacing().interact_size.y * 0.45;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    let rounding = height / 2.0;
    painter.rect_filled(rect, rounding, ui.visuals().faint_bg_color.gamma_multiply(2.0));
    painter.rect_filled(rect, rounding, ui.visuals().extreme_bg_color);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * f).max(2.0));
        painter.rect_filled(fill, rounding, color);
    }
    resp
}

/// Usage bar with a label on top, used for drives.
pub fn usage_bar(ui: &mut Ui, used: u64, total: u64, width: f32) {
    let f = if total == 0 { 0.0 } else { used as f32 / total as f32 };
    let color = if f > 0.9 {
        Color32::from_rgb(0xE0, 0x4F, 0x4F)
    } else if f > 0.75 {
        Color32::from_rgb(0xE0, 0xA0, 0x3C)
    } else {
        Color32::from_rgb(0x3D, 0x8B, 0xE0)
    };
    let height = 14.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let mut fill = rect;
    fill.set_width(rect.width() * f.clamp(0.0, 1.0));
    painter.rect_filled(fill, 4.0, color);
}

/// Standard right-click menu for a node.
pub fn node_context_menu(ui: &mut Ui, ctx: &mut Ctx, id: NodeId) {
    let data = ctx.data;
    let tree = &data.tree;
    let n = tree.get(id);
    let path = tree.path(id);
    let is_dir = n.is_dir;
    let selected = ctx.selection.contains(&id);
    ui.label(RichText::new(n.name.to_string()).strong());
    ui.label(RichText::new(format::size(n.size)).weak());
    ui.separator();
    if ui.button("📂 Open").clicked() {
        ctx.actions.push(UiAction::Open(path.clone()));
        ui.close();
    }
    if ui.button("🔎 Show in file manager").clicked() {
        ctx.actions.push(UiAction::Reveal(path.clone()));
        ui.close();
    }
    if ui.button("🔭 Browse here in PC Scanner").clicked() {
        ctx.actions.push(UiAction::ShowInExplorer(id));
        ui.close();
    }
    if is_dir && ui.button("▦ Show in treemap").clicked() {
        ctx.actions.push(UiAction::ShowInTreemap(id));
        ui.close();
    }
    if ui.button("📋 Copy path").clicked() {
        ctx.actions.push(UiAction::CopyText(path.to_string_lossy().into_owned()));
        ui.close();
    }
    ui.separator();
    let sel_label = if selected { "☐ Remove from selection" } else { "☑ Add to selection" };
    if ui.button(sel_label).clicked() {
        ctx.actions.push(UiAction::ToggleSelect(id));
        ui.close();
    }
    ui.separator();
    if is_dir && ui.button("✨ Empty this folder…").clicked() {
        ctx.actions.push(UiAction::DeleteContents(vec![id]));
        ui.close();
    }
    let red = Color32::from_rgb(0xE0, 0x60, 0x60);
    let multi = selected && ctx.selection.len() > 1;
    if multi {
        // Right-clicking a ticked row acts on every ticked item, like a file manager.
        let label = format!("🗑 Delete {} selected items…", ctx.selection.len());
        if ui.button(RichText::new(label).color(red)).clicked() {
            ctx.actions.push(UiAction::Delete(ctx.selection.iter().copied().collect()));
            ui.close();
        }
        if ui.button(RichText::new("🗑 Delete only this item…").color(red)).clicked() {
            ctx.actions.push(UiAction::Delete(vec![id]));
            ui.close();
        }
    } else if ui.button(RichText::new("🗑 Delete…").color(red)).clicked() {
        ctx.actions.push(UiAction::Delete(vec![id]));
        ui.close();
    }
}

#[derive(Clone, Copy)]
pub struct TableOptions {
    pub show_location: bool,
    pub show_files: bool,
    pub show_age: bool,
    /// Reference total for the percentage bar (0 = no bar).
    pub bar_total: u64,
}

impl Default for TableOptions {
    fn default() -> Self {
        Self { show_location: true, show_files: false, show_age: true, bar_total: 0 }
    }
}

/// Result of interacting with a node table.
#[derive(Default)]
pub struct TableOutput {
    /// A row was double clicked.
    pub activated: Option<NodeId>,
}

/// Virtualised table of nodes with checkbox, name, size, date, location and a context menu.
pub fn node_table(ui: &mut Ui, id_salt: &str, ctx: &mut Ctx, rows: &[NodeId], opts: TableOptions) -> TableOutput {
    let mut out = TableOutput::default();
    let data = ctx.data;
    let row_h = 22.0;
    let mut table = TableBuilder::new(ui)
        .id_salt(id_salt)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(22.0))
        .column(Column::initial(320.0).at_least(120.0).clip(true))
        .column(Column::initial(80.0).at_least(60.0));
    if opts.bar_total > 0 {
        table = table.column(Column::initial(110.0).at_least(60.0));
    }
    if opts.show_files {
        table = table.column(Column::initial(70.0).at_least(50.0));
    }
    if opts.show_age {
        table = table.column(Column::initial(120.0).at_least(80.0));
    }
    if opts.show_location {
        table = table.column(Column::remainder().at_least(100.0).clip(true));
    } else {
        table = table.column(Column::remainder().at_least(10.0));
    }

    table
        .header(20.0, |mut header| {
            header.col(|_| {});
            header.col(|ui| {
                ui.strong("Name");
            });
            header.col(|ui| {
                ui.strong("Size");
            });
            if opts.bar_total > 0 {
                header.col(|ui| {
                    ui.strong("Share");
                });
            }
            if opts.show_files {
                header.col(|ui| {
                    ui.strong("Files");
                });
            }
            if opts.show_age {
                header.col(|ui| {
                    ui.strong("Modified");
                });
            }
            header.col(|ui| {
                if opts.show_location {
                    ui.strong("Location");
                }
            });
        })
        .body(|body| {
            body.rows(row_h, rows.len(), |mut row| {
                let id = rows[row.index()];
                let tree = &data.tree;
                let n = tree.get(id);
                let selected = ctx.selection.contains(&id);
                row.set_selected(selected);
                row.col(|ui| {
                    let mut s = selected;
                    if ui.checkbox(&mut s, "").changed() {
                        ctx.actions.push(UiAction::ToggleSelect(id));
                    }
                });
                row.col(|ui| {
                    let text = format!("{} {}", node_icon(tree, id), n.name);
                    let label = if n.error { RichText::new(text).color(Color32::from_rgb(0xD0, 0x90, 0x40)) } else { RichText::new(text) };
                    ui.add(egui::Label::new(label).truncate().selectable(false));
                });
                row.col(|ui| {
                    ui.label(format::size(n.size));
                });
                if opts.bar_total > 0 {
                    row.col(|ui| {
                        let f = n.size as f32 / opts.bar_total.max(1) as f32;
                        let color = if n.is_dir { Color32::from_rgb(0x3D, 0x8B, 0xE0) } else { n.category.color() };
                        size_bar(ui, f, color, 60.0);
                        ui.label(RichText::new(format!("{:.1}%", f * 100.0)).small());
                    });
                }
                if opts.show_files {
                    row.col(|ui| {
                        if n.is_dir {
                            ui.label(format::count(n.files as u64));
                        }
                    });
                }
                if opts.show_age {
                    row.col(|ui| {
                        ui.label(format::date_short(n.modified)).on_hover_text(format::age(n.modified));
                    });
                }
                row.col(|ui| {
                    if opts.show_location && n.parent != crate::model::tree::NO_PARENT {
                        let loc = tree.path(n.parent).to_string_lossy().into_owned();
                        ui.add(egui::Label::new(RichText::new(loc).weak()).truncate().selectable(false));
                    }
                });
                let resp = row.response();
                if resp.double_clicked() {
                    out.activated = Some(id);
                }
                resp.context_menu(|ui| node_context_menu(ui, ctx, id));
            });
        });
    out
}

/// Placeholder when no scan has been run yet.
pub fn no_scan_placeholder(ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.heading("No scan yet");
        ui.add_space(8.0);
        ui.label("Choose drives in the top bar (or add a folder) and press  ▶ Scan.");
        ui.label(RichText::new("Tip: run as administrator to include protected system folders.").weak());
    });
}

/// A titled stat value used on the overview.
pub fn stat(ui: &mut Ui, title: &str, value: String) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).weak().small());
        ui.label(RichText::new(value).size(18.0).strong());
    });
}
