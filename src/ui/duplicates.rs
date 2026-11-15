//! Duplicate files view.

use std::sync::atomic::Ordering;

use egui::{Color32, RichText, Sense, Ui};

use super::{Ctx, UiAction, node_icon};
use crate::analysis::duplicates::{DupGroup, DupJob};
use crate::util::format;

pub struct DuplicatesState {
    pub min_size_mb: f64,
    pub job: Option<DupJob>,
    pub groups: Option<Vec<DupGroup>>,
    pub generation: u64,
}

impl Default for DuplicatesState {
    fn default() -> Self {
        Self {
            min_size_mb: 1.0,
            job: None,
            groups: None,
            generation: 0,
        }
    }
}

pub fn show(ui: &mut Ui, st: &mut DuplicatesState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    ui.heading("Duplicate files");
    ui.label(
        RichText::new(
            "Files with identical content (compared by size and fast xxHash3 of the contents).",
        )
        .weak(),
    );

    if let Some(job) = &st.job
        && let Some(groups) = job.poll()
    {
        st.groups = Some(groups);
        st.generation = data.generation;
        st.job = None;
    }

    ui.horizontal(|ui| {
        ui.label("Minimum file size (MB)");
        ui.add(
            egui::DragValue::new(&mut st.min_size_mb)
                .range(0.0..=100_000.0)
                .speed(0.5),
        );
        if st.job.is_none() {
            if ui.button("🔍 Find duplicates").clicked() {
                let min = (st.min_size_mb * 1024.0 * 1024.0) as u64;
                st.job = Some(DupJob::start(
                    tree,
                    &data.analysis.files_by_size,
                    min,
                    ui.ctx().clone(),
                ));
                st.groups = None;
            }
        } else if ui.button("⏹ Cancel").clicked()
            && let Some(j) = &st.job
        {
            j.cancel();
        }
    });

    if let Some(job) = &st.job {
        let p = &job.progress;
        let stage = p.stage.load(Ordering::Relaxed);
        let done = p.done.load(Ordering::Relaxed);
        let total = p.total.load(Ordering::Relaxed).max(1);
        let (label, frac) = match stage {
            0 => ("Grouping by size…".to_owned(), 0.0),
            1 => (
                format!(
                    "Quick hashing {} / {} files",
                    format::count(done),
                    format::count(total)
                ),
                done as f32 / total as f32,
            ),
            2 => (
                format!(
                    "Comparing contents {} / {}",
                    format::size(done),
                    format::size(total)
                ),
                done as f32 / total as f32,
            ),
            _ => ("Finishing…".to_owned(), 1.0),
        };
        ui.add(egui::ProgressBar::new(frac).text(label).animate(true));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(150));
        return;
    }

    let Some(groups) = &st.groups else {
        ui.add_space(20.0);
        ui.label("Press “Find duplicates” to start. Large drives can take a few minutes.");
        return;
    };
    let live: Vec<&DupGroup> = groups
        .iter()
        .filter(|g| g.ids.iter().filter(|&&id| !tree.is_gone(id)).count() >= 2)
        .collect();
    let wasted: u64 = live.iter().map(|g| g.wasted()).sum();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} groups · {} can be reclaimed",
                format::count(live.len() as u64),
                format::size(wasted)
            ))
            .strong(),
        );
        if st.generation != data.generation {
            ui.label(RichText::new("(files changed since search)").weak());
        }
        if ui
            .button("☑ Select all but newest in each group")
            .on_hover_text("Keeps the most recently modified copy of each file")
            .clicked()
        {
            let mut ids = Vec::new();
            for g in &live {
                let mut copies: Vec<_> = g
                    .ids
                    .iter()
                    .copied()
                    .filter(|&id| !tree.is_gone(id))
                    .collect();
                copies.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).modified));
                ids.extend(copies.into_iter().skip(1));
            }
            ctx.actions.push(UiAction::Select(ids));
        }
        if ui
            .button("☑ Select all but oldest")
            .on_hover_text("Keeps the original (oldest) copy")
            .clicked()
        {
            let mut ids = Vec::new();
            for g in &live {
                let mut copies: Vec<_> = g
                    .ids
                    .iter()
                    .copied()
                    .filter(|&id| !tree.is_gone(id))
                    .collect();
                copies.sort_by_key(|&id| tree.get(id).modified);
                ids.extend(copies.into_iter().skip(1));
            }
            ctx.actions.push(UiAction::Select(ids));
        }
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, 24.0, live.len(), |ui, range| {
            for gi in range {
                let g = live[gi];
                let first = g.ids[0];
                let header = format!(
                    "{}  ×{}  ·  {} each  ·  {} wasted",
                    tree.get(first).name,
                    g.ids.len(),
                    format::size(g.size),
                    format::size(g.wasted())
                );
                egui::CollapsingHeader::new(RichText::new(header).strong())
                    .id_salt(("dup", gi, first))
                    .show(ui, |ui| {
                        for &id in &g.ids {
                            if tree.is_gone(id) {
                                continue;
                            }
                            let n = tree.get(id);
                            ui.horizontal(|ui| {
                                let mut sel = ctx.selection.contains(&id);
                                if ui.checkbox(&mut sel, "").changed() {
                                    ctx.actions.push(UiAction::ToggleSelect(id));
                                }
                                let path = tree.path(id);
                                let resp = ui.add(
                                    egui::Label::new(format!(
                                        "{} {}",
                                        node_icon(tree, id),
                                        path.display()
                                    ))
                                    .truncate()
                                    .sense(Sense::click()),
                                );
                                ui.label(RichText::new(format::date(n.modified)).weak());
                                if resp.double_clicked() {
                                    ctx.actions.push(UiAction::Open(path));
                                }
                                resp.context_menu(|ui| super::node_context_menu(ui, ctx, id));
                            });
                        }
                    });
            }
        });
    if live.is_empty() {
        ui.label(
            RichText::new("No duplicates found 🎉").color(Color32::from_rgb(0x50, 0xB0, 0x70)),
        );
    }
}
