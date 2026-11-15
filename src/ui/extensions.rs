//! Per-extension statistics.

use egui::{RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use super::{Ctx, TableOptions, UiAction, node_table, size_bar};
use crate::model::NodeId;
use crate::model::category::extension_of;
use crate::util::format;

#[derive(Default)]
pub struct ExtensionsState {
    pub search: String,
    pub selected: Option<String>,
    cache: Vec<NodeId>,
    cache_key: Option<String>,
}

pub fn show(ui: &mut Ui, st: &mut ExtensionsState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    let a = &data.analysis;
    let total = a.total_size.max(1);

    ui.heading("File types");
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Extension, e.g. mp4").desired_width(160.0));
        ui.label(RichText::new(format!("{} different extensions", format::count(a.extensions.len() as u64))).weak());
    });

    let search = st.search.trim_start_matches('.').to_lowercase();
    let exts: Vec<&crate::analysis::ExtStat> =
        a.extensions.iter().filter(|e| search.is_empty() || e.ext.contains(&search)).collect();

    let list_height = if st.selected.is_some() { ui.available_height() * 0.45 } else { ui.available_height() };
    ui.push_id("ext_tbl", |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .max_scroll_height(list_height)
            .column(Column::initial(120.0))
            .column(Column::initial(170.0))
            .column(Column::initial(90.0))
            .column(Column::initial(90.0))
            .column(Column::initial(160.0))
            .column(Column::remainder())
            .header(20.0, |mut h| {
                for t in ["Extension", "Group", "Total size", "Files", "Share", "Largest file"] {
                    h.col(|ui| {
                        ui.strong(t);
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, exts.len(), |mut row| {
                    let e = exts[row.index()];
                    row.set_selected(st.selected.as_deref() == Some(e.ext.as_str()));
                    row.col(|ui| {
                        ui.label(RichText::new(if e.ext.is_empty() { "(none)".to_owned() } else { format!(".{}", e.ext) }).strong());
                    });
                    row.col(|ui| {
                        ui.label(format!("{} {}", e.category.icon(), e.category.label()));
                    });
                    row.col(|ui| {
                        ui.label(format::size(e.size));
                    });
                    row.col(|ui| {
                        ui.label(format::count(e.count));
                    });
                    row.col(|ui| {
                        size_bar(ui, e.size as f32 / total as f32, e.category.color(), 90.0);
                        ui.label(RichText::new(format!("{:.1}%", format::percent(e.size, total))).small());
                    });
                    row.col(|ui| {
                        if !tree.get(e.largest).deleted {
                            ui.add(egui::Label::new(RichText::new(format!("{} ({})", tree.get(e.largest).name, format::size(tree.get(e.largest).size))).weak()).truncate().selectable(false));
                        }
                    });
                    if row.response().clicked() {
                        st.selected = if st.selected.as_deref() == Some(e.ext.as_str()) { None } else { Some(e.ext.clone()) };
                    }
                });
            });
    });

    if let Some(ext) = st.selected.clone() {
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong(format!("Files with .{}", if ext.is_empty() { "(no extension)" } else { &ext }));
            if ui.button("✖ Close").clicked() {
                st.selected = None;
            }
            if ui.button("☑ Select all listed").clicked() {
                ctx.actions.push(UiAction::Select(st.cache.clone()));
            }
        });
        let key = format!("{}|{}", data.generation, ext);
        if st.cache_key.as_ref() != Some(&key) {
            st.cache = a
                .files_by_size
                .iter()
                .copied()
                .filter(|&id| !tree.get(id).deleted && extension_of(&tree.get(id).name) == ext)
                .take(20_000)
                .collect();
            st.cache_key = Some(key);
        }
        let rows = std::mem::take(&mut st.cache);
        let out = node_table(ui, "ext_files", ctx, &rows, TableOptions::default());
        if let Some(id) = out.activated {
            ctx.actions.push(UiAction::Open(tree.path(id)));
        }
        st.cache = rows;
    }
}
