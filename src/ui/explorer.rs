//! Drill-down folder browser, sorted by size.

use egui::{RichText, Ui};

use super::{Ctx, TableOptions, UiAction, node_table};
use crate::model::NodeId;
use crate::util::format;

#[derive(Default)]
pub struct ExplorerState {
    /// Current folder; None = list of scanned roots.
    pub current: Option<NodeId>,
    pub filter: String,
    pub history: Vec<Option<NodeId>>,
    pub sort: SortBy,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    #[default]
    Size,
    Name,
    Modified,
    Files,
}

impl ExplorerState {
    pub fn navigate(&mut self, to: Option<NodeId>) {
        if self.current != to {
            self.history.push(self.current);
            self.current = to;
            self.filter.clear();
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut ExplorerState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    if let Some(cur) = state.current
        && tree.is_gone(cur)
    {
        state.current = Some(tree.get(cur).parent).filter(|&p| p != crate::model::tree::NO_PARENT);
    }

    // Toolbar + breadcrumbs
    ui.horizontal(|ui| {
        if ui.add_enabled(!state.history.is_empty(), egui::Button::new("⬅")).on_hover_text("Back").clicked()
            && let Some(prev) = state.history.pop()
        {
            state.current = prev;
        }
        let parent = state.current.map(|c| tree.get(c).parent).filter(|&p| p != crate::model::tree::NO_PARENT);
        if ui.add_enabled(state.current.is_some(), egui::Button::new("⬆")).on_hover_text("Up").clicked() {
            state.navigate(parent);
        }
        if ui.button("🏠").on_hover_text("All scanned locations").clicked() {
            state.navigate(None);
        }
        ui.separator();
        if ui.link("This PC").clicked() {
            state.navigate(None);
        }
        if let Some(cur) = state.current {
            for id in tree.ancestry(cur) {
                ui.label("›");
                let name = tree.get(id).name.to_string();
                if ui.link(name).clicked() {
                    state.navigate(Some(id));
                }
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut state.filter).hint_text("Filter this folder").desired_width(220.0));
        ui.label("Sort:");
        ui.selectable_value(&mut state.sort, SortBy::Size, "Size");
        ui.selectable_value(&mut state.sort, SortBy::Name, "Name");
        ui.selectable_value(&mut state.sort, SortBy::Files, "Files");
        ui.selectable_value(&mut state.sort, SortBy::Modified, "Modified");
        if let Some(cur) = state.current {
            ui.separator();
            let n = tree.get(cur);
            ui.label(RichText::new(format!("{} · {} files", format::size(n.size), format::count(n.files as u64))).strong());
            if ui.button("📂 Open").clicked() {
                ctx.actions.push(UiAction::Open(tree.path(cur)));
            }
            if ui.button("🔎 Show in file manager").clicked() {
                ctx.actions.push(UiAction::Reveal(tree.path(cur)));
            }
            if ui.button("▦ Treemap").clicked() {
                ctx.actions.push(UiAction::ShowInTreemap(cur));
            }
        }
    });
    ui.separator();

    let mut rows: Vec<NodeId> = match state.current {
        Some(cur) => tree.children(cur).collect(),
        None => tree.roots.iter().copied().filter(|&r| !tree.get(r).deleted).collect(),
    };
    if !state.filter.is_empty() {
        let f = state.filter.to_lowercase();
        rows.retain(|&id| tree.get(id).name.to_lowercase().contains(&f));
    }
    match state.sort {
        SortBy::Size => {} // already sorted by size
        SortBy::Name => rows.sort_by_cached_key(|&id| (!tree.get(id).is_dir, tree.get(id).name.to_lowercase())),
        SortBy::Modified => rows.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).modified)),
        SortBy::Files => rows.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).files)),
    }
    let total = match state.current {
        Some(c) => tree.get(c).size,
        None => tree.total_size(),
    };
    if rows.is_empty() {
        ui.label(RichText::new("This folder is empty.").weak());
        return;
    }
    let out = node_table(
        ui,
        "explorer_table",
        ctx,
        &rows,
        TableOptions { show_location: false, show_files: true, show_age: true, bar_total: total.max(1) },
    );
    if let Some(id) = out.activated {
        if tree.get(id).is_dir {
            state.navigate(Some(id));
        } else {
            ctx.actions.push(UiAction::Open(tree.path(id)));
        }
    }
}
