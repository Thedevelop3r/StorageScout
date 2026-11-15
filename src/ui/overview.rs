//! Dashboard: space ring by category, storage health, drive gauges, quick wins and the
//! largest folders.

use std::collections::BTreeSet;

use egui::{Color32, RichText, Sense, Ui, vec2};

use super::theme::{self, pal};
use super::{Ctx, ScanData, UiAction, icons};
use crate::analysis::cleanup::Risk;
use crate::app::View;
use crate::model::{Category, NodeId};
use crate::system::drives::DriveInfo;
use crate::util::format;

const GAP: f32 = 20.0;

pub fn show(
    ui: &mut Ui,
    drives: &[DriveInfo],
    data: Option<&ScanData>,
    selection: &BTreeSet<NodeId>,
    actions: &mut Vec<UiAction>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let avail = ui.available_width() - 8.0;
            let wide = avail >= 980.0;
            let left_w = if wide { (avail - GAP) * 0.6 } else { avail };
            let right_w = if wide { avail - GAP - left_w } else { avail };

            let Some(data) = data else {
                welcome(ui, avail, actions);
                ui.add_space(GAP);
                drive_row(ui, drives, None, avail);
                return;
            };
            let mut ctx_holder = Ctx {
                data,
                selection,
                actions: &mut *actions,
            };
            let ctx = &mut ctx_holder;

            let top = |ui: &mut Ui, ctx_actions: &mut Vec<UiAction>| {
                ui.vertical(|ui| {
                    ui.set_width(left_w);
                    hero(ui, data, left_w);
                });
                if wide {
                    ui.add_space(GAP);
                }
                ui.vertical(|ui| {
                    ui.set_width(right_w);
                    health(ui, drives, data, right_w, ctx_actions);
                    ui.add_space(GAP);
                    drive_row(ui, drives, Some(data), right_w);
                });
            };
            if wide {
                ui.horizontal_top(|ui| top(ui, ctx.actions));
            } else {
                ui.vertical(|ui| top(ui, ctx.actions));
            }
            ui.add_space(GAP);

            let half = if wide { (avail - GAP) / 2.0 } else { avail };
            let bottom = |ui: &mut Ui, ctx: &mut Ctx| {
                ui.vertical(|ui| {
                    ui.set_width(half);
                    quick_wins(ui, data, half, ctx.actions);
                });
                if wide {
                    ui.add_space(GAP);
                }
                ui.vertical(|ui| {
                    ui.set_width(half);
                    largest_folders(ui, ctx, half);
                });
            };
            if wide {
                ui.horizontal_top(|ui| bottom(ui, ctx));
            } else {
                ui.vertical(|ui| bottom(ui, ctx));
            }
            ui.add_space(GAP);
        });
}

fn welcome(ui: &mut Ui, width: f32, actions: &mut Vec<UiAction>) {
    let p = pal(ui);
    theme::card(ui).show(ui, |ui| {
        ui.set_width(width - 42.0);
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(icons::HARD_DRIVES)
                    .size(44.0)
                    .color(p.accent_text),
            );
            ui.add_space(8.0);
            theme::title(ui, "See what is using space on your PC", 26.0);
            ui.label(
                RichText::new(
                    "Pick drives in the top bar, or add a folder, then start a scan. \
                     Run as administrator to include protected system folders.",
                )
                .color(p.text_dim),
            );
            ui.add_space(14.0);
            if theme::primary_button(ui, icons::PLAY, "Start scan").clicked() {
                actions.push(UiAction::StartScan);
            }
        });
        ui.add_space(16.0);
    });
}

fn hero(ui: &mut Ui, data: &ScanData, width: f32) {
    let p = pal(ui);
    let a = &data.analysis;
    let total = a.category_total().max(1);
    let mut cats: Vec<Category> = Category::ALL
        .iter()
        .copied()
        .filter(|c| a.categories[c.index()].size > 0)
        .collect();
    cats.sort_by_key(|c| std::cmp::Reverse(a.categories[c.index()].size));
    let shown: Vec<Category> = cats.iter().copied().take(7).collect();
    let rest: u64 = cats
        .iter()
        .skip(7)
        .map(|c| a.categories[c.index()].size)
        .sum();
    let mut segments: Vec<(f32, Color32)> = shown
        .iter()
        .map(|c| {
            (
                a.categories[c.index()].size as f32 / total as f32,
                c.color(),
            )
        })
        .collect();
    if rest > 0 {
        segments.push((rest as f32 / total as f32, Category::Other.color()));
    }

    theme::card(ui).show(ui, |ui| {
        ui.set_width(width - 42.0);
        let ring = 250.0;
        let layout = |ui: &mut Ui| {
            let (resp, hovered) = theme::donut(ui, ring, 26.0, &segments);
            let c = resp.rect.center();
            let painter = ui.painter();
            match hovered {
                Some(i) if i < shown.len() => {
                    let cat = shown[i];
                    let st = a.categories[cat.index()];
                    painter.text(
                        c - vec2(0.0, 22.0),
                        egui::Align2::CENTER_CENTER,
                        cat.label(),
                        egui::FontId::proportional(13.0),
                        p.text_muted,
                    );
                    painter.text(
                        c + vec2(0.0, 6.0),
                        egui::Align2::CENTER_CENTER,
                        format::size(st.size),
                        theme::heading(32.0),
                        p.text,
                    );
                    painter.text(
                        c + vec2(0.0, 34.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{} files", format::count(st.count)),
                        egui::FontId::proportional(12.0),
                        p.text_muted,
                    );
                }
                _ => {
                    painter.text(
                        c - vec2(0.0, 22.0),
                        egui::Align2::CENTER_CENTER,
                        "Scanned",
                        egui::FontId::proportional(13.0),
                        p.text_muted,
                    );
                    painter.text(
                        c + vec2(0.0, 6.0),
                        egui::Align2::CENTER_CENTER,
                        format::size(a.total_size),
                        theme::heading(36.0),
                        p.text,
                    );
                    painter.text(
                        c + vec2(0.0, 34.0),
                        egui::Align2::CENTER_CENTER,
                        format!(
                            "{} files · {}",
                            format::count(a.total_files),
                            format::duration(data.elapsed_secs)
                        ),
                        egui::FontId::proportional(12.0),
                        p.text_muted,
                    );
                }
            }
            ui.add_space(24.0);
            ui.vertical(|ui| {
                theme::title(ui, "Your PC at a glance", 24.0);
                let lead = match (shown.first(), shown.get(1)) {
                    (Some(a1), Some(b1)) => format!(
                        "{} and {} use {:.0}% of the scanned space.",
                        a1.label(),
                        b1.label().to_lowercase(),
                        (segments[0].0 + segments[1].0) * 100.0
                    ),
                    _ => "Here is what uses the scanned space.".into(),
                };
                ui.label(RichText::new(lead).color(p.text_dim));
                if data.cancelled {
                    ui.label(
                        RichText::new(format!(
                            "{}  The scan was cancelled, so results are partial.",
                            icons::WARNING
                        ))
                        .color(p.warning),
                    );
                }
                ui.add_space(8.0);
                let row = |ui: &mut Ui, color: Color32, label: &str, size: u64| {
                    ui.horizontal(|ui| {
                        theme::swatch(ui, color);
                        ui.label(RichText::new(label).color(p.text_dim));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format::size(size)).font(theme::bold(13.5)));
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(format!("{:.1}%", format::percent(size, total)))
                                    .color(p.text_muted),
                            );
                        });
                    });
                };
                for c in &shown {
                    row(ui, c.color(), c.label(), a.categories[c.index()].size);
                }
                if rest > 0 {
                    row(ui, Category::Other.color(), "Everything else", rest);
                }
            });
        };
        if width > 640.0 {
            ui.horizontal_top(layout);
        } else {
            ui.vertical_centered(layout);
        }
    });
}

/// 0..=100 score from drive fullness and cleanup potential.
fn health_score(drives: &[DriveInfo], data: &ScanData) -> (u32, &'static str) {
    let mut score = 100.0f32;
    for d in drives.iter().filter(|d| !d.removable && d.total > 0) {
        let used = d.used() as f32 / d.total as f32 * 100.0;
        if used > 75.0 {
            score -= (used - 75.0) * 1.6;
        }
    }
    let reclaim: u64 = data.cleanup.iter().map(|c| c.size).sum();
    let used_total: u64 = drives
        .iter()
        .filter(|d| !d.removable)
        .map(|d| d.used())
        .sum();
    if used_total > 0 {
        score -= (reclaim as f32 / used_total as f32 * 100.0 * 2.0).min(20.0);
    }
    let s = score.clamp(0.0, 100.0).round() as u32;
    let label = match s {
        90..=100 => "Excellent",
        75..=89 => "Good, with room to tidy",
        50..=74 => "Needs attention",
        _ => "Running out of space",
    };
    (s, label)
}

fn health(
    ui: &mut Ui,
    drives: &[DriveInfo],
    data: &ScanData,
    width: f32,
    actions: &mut Vec<UiAction>,
) {
    let p = pal(ui);
    let (score, label) = health_score(drives, data);
    let color = match score {
        75..=100 => p.success,
        50..=74 => p.warning,
        _ => p.danger,
    };
    let reclaim: u64 = data.cleanup.iter().map(|c| c.size).sum();
    theme::card(ui).show(ui, |ui| {
        ui.set_width(width - 42.0);
        ui.horizontal(|ui| {
            theme::gauge(
                ui,
                92.0,
                9.0,
                score as f32 / 100.0,
                color,
                &score.to_string(),
            );
            ui.add_space(14.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Storage health")
                        .size(12.0)
                        .color(p.text_muted),
                );
                ui.label(RichText::new(label).font(theme::bold(18.0)).color(p.text));
                let link = ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{} can be freed  {}",
                            format::size(reclaim),
                            icons::ARROW_RIGHT
                        ))
                        .color(p.accent_text),
                    )
                    .sense(Sense::click()),
                );
                if link.clicked() {
                    actions.push(UiAction::Navigate(View::Cleanup));
                }
            });
        });
    });
}

fn drive_row(ui: &mut Ui, drives: &[DriveInfo], data: Option<&ScanData>, width: f32) {
    let p = pal(ui);
    let per_row = ((width + GAP) / (220.0 + GAP)).floor().max(1.0) as usize;
    let count = drives.len().max(1);
    let cols = per_row.min(count);
    let card_w = (width - GAP * (cols as f32 - 1.0)) / cols as f32;
    let colors = [p.accent, p.violet, p.success, p.warning];
    for (row_i, chunk) in drives.chunks(cols).enumerate() {
        if row_i > 0 {
            ui.add_space(GAP);
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for (i, d) in chunk.iter().enumerate() {
                let color = colors[(row_i * cols + i) % colors.len()];
                let frac = if d.total > 0 {
                    d.used() as f32 / d.total as f32
                } else {
                    0.0
                };
                let gauge_color = if frac > 0.9 {
                    p.danger
                } else if frac > 0.8 {
                    p.warning
                } else {
                    color
                };
                theme::card(ui).show(ui, |ui| {
                    ui.set_width(card_w - 42.0);
                    ui.vertical(|ui| drive_card(ui, d, data, frac, gauge_color));
                });
            }
        });
    }
    if drives.is_empty() {
        ui.label(RichText::new("No drives found.").color(p.text_muted));
    }
}

fn drive_card(
    ui: &mut Ui,
    d: &DriveInfo,
    data: Option<&ScanData>,
    frac: f32,
    gauge_color: Color32,
) {
    let p = pal(ui);
    ui.horizontal(|ui| {
        theme::gauge(
            ui,
            64.0,
            7.0,
            frac,
            gauge_color,
            &format!("{:.0}%", frac * 100.0),
        );
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(d.display_name())
                    .font(theme::bold(15.0))
                    .color(p.text),
            )
            .on_hover_text(format!("File system: {}", d.file_system));
            let removable = if d.removable { " · removable" } else { "" };
            ui.label(
                RichText::new(format!(
                    "{} · {}{}",
                    d.kind,
                    format::size(d.total),
                    removable
                ))
                .size(12.0)
                .color(p.text_muted),
            );
        });
    });
    ui.add_space(4.0);
    ui.label(RichText::new(format!("{} free", format::size(d.available))).color(p.text_dim));
    if let Some(data) = data
        && let Some(root) = data.tree.find_path(&d.mount)
    {
        let scanned = data.tree.get(root).size;
        let hidden = d.used().saturating_sub(scanned);
        let mut text = format!("Scanned {}", format::size(scanned));
        if hidden > d.total / 50 {
            text.push_str(&format!(" · {} not visible", format::size(hidden)));
        }
        ui.label(RichText::new(text).size(12.0).color(p.text_muted))
            .on_hover_text(
                "Space the scan cannot see: restore points, shadow copies, file \
                                 system metadata and folders that need administrator rights.",
            );
    }
}

fn quick_wins(ui: &mut Ui, data: &ScanData, width: f32, actions: &mut Vec<UiAction>) {
    let p = pal(ui);
    let mut items: Vec<&crate::analysis::cleanup::CleanupItem> =
        data.cleanup.iter().filter(|c| c.size > 0).collect();
    items.sort_by_key(|c| std::cmp::Reverse(c.size));
    theme::card(ui).show(ui, |ui| {
        ui.set_width(width - 42.0);
        ui.horizontal(|ui| {
            theme::title(ui, "Quick wins", 18.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::secondary_button(ui, "Review all").clicked() {
                    actions.push(UiAction::Navigate(View::Cleanup));
                }
            });
        });
        ui.add_space(6.0);
        if items.is_empty() {
            ui.label(RichText::new("Nothing to clean up. Nice and tidy.").color(p.text_muted));
        }
        for item in items.iter().take(4) {
            let (fg, bg) = match item.risk {
                Risk::Safe => (p.accent_text, p.accent_bg),
                Risk::Review => (p.warning, p.warning_bg),
            };
            theme::tile(ui).show(ui, |ui| {
                ui.set_width(width - 42.0 - 30.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                    ui.painter().rect_filled(rect, 10.0, bg);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if matches!(item.risk, Risk::Safe) {
                            icons::SPARKLE
                        } else {
                            icons::WARNING
                        },
                        egui::FontId::proportional(18.0),
                        fg,
                    );
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&item.title)
                                    .font(theme::bold(14.0))
                                    .color(p.text),
                            )
                            .truncate(),
                        );
                        let sub = format!(
                            "{} · {} location{}",
                            item.group,
                            item.targets.len(),
                            if item.targets.len() == 1 { "" } else { "s" }
                        );
                        ui.label(RichText::new(sub).size(12.0).color(p.text_muted));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::secondary_button(ui, "Select").clicked() {
                            actions.push(UiAction::Select(item.targets.clone()));
                            actions.push(UiAction::Notify(format!(
                                "Selected {} ({}). Delete it from the bar at the bottom.",
                                item.title,
                                format::size(item.size)
                            )));
                        }
                        ui.label(
                            RichText::new(format::size(item.size))
                                .font(theme::heading(17.0))
                                .color(p.text),
                        );
                    });
                });
            });
            ui.add_space(6.0);
        }
    });
}

fn largest_folders(ui: &mut Ui, ctx: &mut Ctx, width: f32) {
    let p = pal(ui);
    let data = ctx.data;
    let tree = &data.tree;
    let ids: Vec<crate::model::NodeId> = data
        .analysis
        .dirs_by_size
        .iter()
        .copied()
        .filter(|&id| {
            let n = tree.get(id);
            !n.deleted && n.parent != crate::model::tree::NO_PARENT
        })
        .take(7)
        .collect();
    let max = ids.first().map(|&id| tree.get(id).size).unwrap_or(1).max(1);
    theme::card(ui).show(ui, |ui| {
        ui.set_width(width - 42.0);
        ui.horizontal(|ui| {
            theme::title(ui, "Largest folders", 18.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if theme::secondary_button(ui, "Explore").clicked() {
                    ctx.actions.push(UiAction::Navigate(View::LargestFolders));
                }
            });
        });
        ui.add_space(6.0);
        for id in ids {
            let n = tree.get(id);
            let path = tree.path(id).to_string_lossy().into_owned();
            let resp = ui
                .horizontal(|ui| {
                    ui.label(RichText::new(icons::FOLDER).color(p.text_muted));
                    let label_w = (width - 42.0 - 200.0).max(80.0);
                    ui.allocate_ui(vec2(label_w, 20.0), |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&path).color(p.text_dim))
                                .truncate()
                                .selectable(false),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format::size(n.size)).font(theme::bold(13.5)));
                        theme::bar(ui, 70.0, 5.0, n.size as f32 / max as f32, p.accent);
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_text("Double-click to browse, right-click for actions");
            if resp.double_clicked() {
                ctx.actions.push(UiAction::ShowInExplorer(id));
            }
            resp.context_menu(|ui| super::node_context_menu(ui, ctx, id));
            ui.add_space(4.0);
        }
    });
}
