//! Browse files grouped by category, with per-extension breakdown.

use egui::{RichText, Sense, Ui, Vec2};

use super::{Ctx, TableOptions, UiAction, node_table, size_bar};
use crate::model::category::extension_of;
use crate::model::{Category, NodeId};
use crate::util::format;

#[derive(Default)]
pub struct CategoriesState {
    pub selected: Option<Category>,
    pub ext: Option<String>,
    pub search: String,
    cache: Vec<NodeId>,
    cache_key: Option<String>,
}

pub fn show(ui: &mut Ui, st: &mut CategoriesState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    let a = &data.analysis;
    let total = a.category_total().max(1);

    let mut cats: Vec<Category> = Category::ALL.iter().copied().filter(|c| a.categories[c.index()].count > 0).collect();
    cats.sort_by_key(|c| std::cmp::Reverse(a.categories[c.index()].size));
    if st.selected.is_none() {
        st.selected = cats.first().copied();
    }

    egui::Panel::left("cat_list").resizable(true).default_size(300.0).show(ui, |ui| {
        ui.heading("Groups");
        ui.add_space(4.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for c in &cats {
                let stc = a.categories[c.index()];
                let selected = st.selected == Some(*c);
                let resp = egui::Frame::NONE
                    .fill(if selected { ui.visuals().selection.bg_fill.gamma_multiply(0.5) } else { egui::Color32::TRANSPARENT })
                    .inner_margin(6.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                            ui.painter().rect_filled(r, 2.0, c.color());
                            ui.label(RichText::new(format!("{} {}", c.icon(), c.label())).strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(format::size(stc.size));
                            });
                        });
                        ui.horizontal(|ui| {
                            size_bar(ui, stc.size as f32 / total as f32, c.color(), 150.0);
                            ui.label(RichText::new(format!("{} files · {:.1}%", format::count(stc.count), format::percent(stc.size, total))).small().weak());
                        });
                    })
                    .response
                    .interact(Sense::click());
                if resp.clicked() {
                    st.selected = Some(*c);
                    st.ext = None;
                }
            }
        });
    });

    let Some(cat) = st.selected else { return };
    let cstat = a.categories[cat.index()];
    ui.heading(format!("{} {}", cat.icon(), cat.label()));
    ui.label(format!("{} in {} files", format::size(cstat.size), format::count(cstat.count)));

    // Extension chips for this category
    let exts: Vec<_> = a.extensions.iter().filter(|e| e.category == cat).take(40).collect();
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(st.ext.is_none(), "All").clicked() {
            st.ext = None;
        }
        for e in &exts {
            let label = format!(".{} · {}", if e.ext.is_empty() { "(none)" } else { &e.ext }, format::size(e.size));
            if ui.selectable_label(st.ext.as_deref() == Some(e.ext.as_str()), label).on_hover_text(format!("{} files", format::count(e.count))).clicked() {
                st.ext = Some(e.ext.clone());
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Filter by name").desired_width(220.0));
    });

    let key = format!("{}|{:?}|{:?}|{}", data.generation, cat, st.ext, st.search);
    if st.cache_key.as_ref() != Some(&key) {
        let search = st.search.to_lowercase();
        st.cache = a
            .files_by_size
            .iter()
            .copied()
            .filter(|&id| {
                let n = tree.get(id);
                !n.deleted
                    && n.category == cat
                    && st.ext.as_ref().is_none_or(|e| extension_of(&n.name) == *e)
                    && (search.is_empty() || n.name.to_lowercase().contains(&search))
            })
            .take(20_000)
            .collect();
        st.cache_key = Some(key);
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} files listed", format::count(st.cache.len() as u64))).weak());
        if ui.button("☑ Select all listed").clicked() {
            ctx.actions.push(UiAction::Select(st.cache.clone()));
        }
    });
    ui.separator();
    let rows = std::mem::take(&mut st.cache);
    let out = node_table(ui, "cat_table", ctx, &rows, TableOptions::default());
    if let Some(id) = out.activated {
        ctx.actions.push(UiAction::Open(tree.path(id)));
    }
    st.cache = rows;
}
