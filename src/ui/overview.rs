//! Overview: drives, scan summary, category breakdown, top folders and files.

use egui::{Color32, RichText, Sense, Ui, Vec2};

use super::{Ctx, ScanData, UiAction, node_icon, size_bar, stat, usage_bar};
use crate::model::Category;
use crate::system::drives::DriveInfo;
use crate::util::format;

pub fn show(ui: &mut Ui, drives: &[DriveInfo], data: Option<&ScanData>, ctx: Option<&mut Ctx>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.heading("Drives");
            ui.add_space(4.0);
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Min).with_main_wrap(true),
                |ui| {
                    for d in drives {
                        drive_card(ui, d, data);
                    }
                    if drives.is_empty() {
                        ui.label("No drives found.");
                    }
                },
            );
            ui.add_space(12.0);

            let (Some(data), Some(ctx)) = (data, ctx) else {
                super::no_scan_placeholder(ui);
                return;
            };
            let a = &data.analysis;

            ui.separator();
            ui.heading("Scan summary");
            ui.horizontal(|ui| {
                stat(ui, "Scanned size", format::size(a.total_size));
                ui.add_space(24.0);
                stat(ui, "Files", format::count(a.total_files));
                ui.add_space(24.0);
                stat(ui, "Folders", format::count(a.total_dirs));
                ui.add_space(24.0);
                stat(ui, "Scan time", format::duration(data.elapsed_secs));
                ui.add_space(24.0);
                stat(ui, "Inaccessible", format::count(data.errors.len() as u64));
                ui.add_space(24.0);
                let reclaim: u64 = data.cleanup.iter().map(|c| c.size).sum();
                stat(ui, "Cleanup potential", format::size(reclaim));
            });
            if data.cancelled {
                ui.label(
                    RichText::new("⚠ Scan was cancelled — results are partial.")
                        .color(Color32::from_rgb(0xE0, 0xA0, 0x3C)),
                );
            }
            ui.add_space(12.0);

            // Category stacked bar
            ui.heading("What is using space");
            let total = a.category_total().max(1);
            let width = ui.available_width().min(1100.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
            let mut x = rect.left();
            let mut hovered: Option<Category> = None;
            let mut cats: Vec<Category> = Category::ALL.to_vec();
            cats.sort_by_key(|c| std::cmp::Reverse(a.categories[c.index()].size));
            for c in &cats {
                let s = a.categories[c.index()].size;
                if s == 0 {
                    continue;
                }
                let w = rect.width() * s as f32 / total as f32;
                let r = egui::Rect::from_min_size(
                    egui::pos2(x, rect.top()),
                    Vec2::new(w, rect.height()),
                );
                ui.painter().rect_filled(r, 0.0, c.color());
                if let Some(p) = resp.hover_pos()
                    && r.contains(p)
                {
                    hovered = Some(*c);
                }
                x += w;
            }
            if let Some(c) = hovered {
                let st = a.categories[c.index()];
                resp.on_hover_text(format!(
                    "{} {}\n{} in {} files ({:.1}%)",
                    c.icon(),
                    c.label(),
                    format::size(st.size),
                    format::count(st.count),
                    format::percent(st.size, total)
                ));
            }
            ui.add_space(6.0);
            egui::Grid::new("cat_grid")
                .num_columns(5)
                .spacing([16.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    for c in &cats {
                        let st = a.categories[c.index()];
                        if st.count == 0 {
                            continue;
                        }
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                        ui.painter().rect_filled(r, 2.0, c.color());
                        ui.label(format!("{} {}", c.icon(), c.label()));
                        ui.label(RichText::new(format::size(st.size)).strong());
                        ui.label(format!("{} files", format::count(st.count)));
                        size_bar(ui, st.size as f32 / total as f32, c.color(), 160.0);
                        ui.end_row();
                    }
                });
            ui.add_space(16.0);

            ui.columns(2, |cols| {
                cols[0].heading("Largest folders");
                top_list(&mut cols[0], ctx, &a.dirs_by_size, true);
                cols[1].heading("Largest files");
                top_list(&mut cols[1], ctx, &a.files_by_size, false);
            });
        });
}

fn top_list(ui: &mut Ui, ctx: &mut Ctx, ids: &[crate::model::NodeId], skip_roots: bool) {
    let data = ctx.data;
    let tree = &data.tree;
    let mut shown = 0;
    let max = ids
        .iter()
        .find(|&&id| !(skip_roots && tree.get(id).parent == crate::model::tree::NO_PARENT))
        .map(|&id| tree.get(id).size)
        .unwrap_or(1)
        .max(1);
    for &id in ids {
        if shown >= 12 {
            break;
        }
        let n = tree.get(id);
        if n.deleted || (skip_roots && n.parent == crate::model::tree::NO_PARENT) {
            continue;
        }
        shown += 1;
        ui.horizontal(|ui| {
            let color = if n.is_dir {
                Color32::from_rgb(0x3D, 0x8B, 0xE0)
            } else {
                n.category.color()
            };
            size_bar(ui, n.size as f32 / max as f32, color, 70.0);
            ui.label(RichText::new(format::size(n.size)).strong().monospace());
            let path = tree.path(id).to_string_lossy().into_owned();
            let resp = ui
                .add(
                    egui::Label::new(format!("{} {}", node_icon(tree, id), path))
                        .truncate()
                        .sense(Sense::click()),
                )
                .on_hover_text("Double-click to browse, right-click for actions");
            if resp.double_clicked() {
                ctx.actions.push(UiAction::ShowInExplorer(id));
            }
            resp.context_menu(|ui| super::node_context_menu(ui, ctx, id));
        });
    }
}

fn drive_card(ui: &mut Ui, d: &DriveInfo, data: Option<&ScanData>) {
    egui::Frame::group(ui.style()).inner_margin(10.0).show(ui, |ui| {
        ui.set_width(260.0);
        ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("💽").size(22.0));
            ui.vertical(|ui| {
                ui.label(RichText::new(d.display_name()).strong());
                let removable = if d.removable { " · removable" } else { "" };
                ui.label(RichText::new(format!("{} · {}{}", d.file_system, d.kind, removable)).weak().small());
            });
        });
        usage_bar(ui, d.used(), d.total, 240.0);
        ui.label(format!(
            "{} free of {}  ({:.0}% used)",
            format::size(d.available),
            format::size(d.total),
            format::percent(d.used(), d.total)
        ));
        if let Some(data) = data
            && let Some(root) = data.tree.find_path(&d.mount)
        {
            let scanned = data.tree.get(root).size;
            let hidden = d.used().saturating_sub(scanned);
            ui.label(RichText::new(format!("Scanned: {}", format::size(scanned))).small());
            if hidden > d.total / 50 {
                ui.label(RichText::new(format!("Not visible to scan: {}", format::size(hidden))).small().weak())
                    .on_hover_text("System restore points, shadow copies, file system metadata,\nand folders that need administrator rights.");
            }
        }
        });
    });
}
