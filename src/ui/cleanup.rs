//! Cleanup suggestions view.

use egui::{Color32, RichText, Ui};

use super::{Ctx, UiAction, node_icon};
use crate::analysis::cleanup::{CleanKind, Risk};
use crate::util::format;

#[derive(Default)]
pub struct CleanupState {
    pub expanded: Option<usize>,
}

pub fn show(ui: &mut Ui, st: &mut CleanupState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    let items = &data.cleanup;
    let total: u64 = items.iter().map(|i| i.size).sum();
    let safe: u64 = items.iter().filter(|i| i.risk == Risk::Safe).map(|i| i.size).sum();

    ui.heading("Cleanup");
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Up to {} can be freed", format::size(total))).strong().size(16.0));
        ui.label(RichText::new(format!("({} marked safe)", format::size(safe))).weak());
    });
    ui.label(RichText::new("Close programs (especially browsers) before cleaning their caches. Files that are in use are skipped automatically.").weak());
    ui.separator();

    if items.is_empty() {
        ui.label("Nothing to clean up was found in the scanned locations.");
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let mut last_group = "";
        for (i, item) in items.iter().enumerate() {
            let live: Vec<_> = item.targets.iter().copied().filter(|&t| !tree.is_gone(t)).collect();
            let size: u64 = live.iter().map(|&t| tree.get(t).size).sum();
            if size == 0 {
                continue;
            }
            if item.group != last_group {
                ui.add_space(8.0);
                ui.label(RichText::new(item.group).heading());
                last_group = item.group;
            }
            egui::Frame::group(ui.style()).inner_margin(8.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (risk_color, risk_text) = match item.risk {
                        Risk::Safe => (Color32::from_rgb(0x4C, 0xB0, 0x6C), Risk::Safe.label()),
                        Risk::Review => (Color32::from_rgb(0xE0, 0xA0, 0x3C), Risk::Review.label()),
                    };
                    ui.label(RichText::new(format!(" {risk_text} ")).background_color(risk_color).color(Color32::BLACK).small());
                    ui.label(RichText::new(&item.title).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let verb = match item.kind {
                            CleanKind::Contents => "🧹 Clean",
                            CleanKind::Whole => "🗑 Remove",
                        };
                        if ui.button(verb).clicked() {
                            ctx.actions.push(match item.kind {
                                CleanKind::Contents => UiAction::DeleteContents(live.clone()),
                                CleanKind::Whole => UiAction::Delete(live.clone()),
                            });
                        }
                        if ui.button("☑ Select").on_hover_text("Add to the selection for a combined cleanup").clicked() {
                            ctx.actions.push(UiAction::Select(live.clone()));
                        }
                        let open = st.expanded == Some(i);
                        if ui.selectable_label(open, format!("{} location(s)", live.len())).clicked() {
                            st.expanded = if open { None } else { Some(i) };
                        }
                        ui.label(RichText::new(format::size(size)).strong().size(15.0));
                    });
                });
                ui.label(RichText::new(item.description).weak());
                if st.expanded == Some(i) {
                    ui.separator();
                    egui::ScrollArea::vertical().id_salt(("cl", i)).max_height(240.0).show(ui, |ui| {
                        for &t in &live {
                            ui.horizontal(|ui| {
                                let path = tree.path(t);
                                ui.label(RichText::new(format::size(tree.get(t).size)).monospace());
                                let resp = ui.add(
                                    egui::Label::new(format!("{} {}", node_icon(tree, t), path.display()))
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                );
                                if resp.double_clicked() {
                                    ctx.actions.push(UiAction::ShowInExplorer(t));
                                }
                                resp.context_menu(|ui| super::node_context_menu(ui, ctx, t));
                            });
                        }
                    });
                }
            });
        }
    });
}
