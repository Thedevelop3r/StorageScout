//! Full-screen scan progress: big progress rings, live counters and the path being read.

use std::sync::atomic::Ordering;
use std::time::Duration;

use egui::{RichText, Ui, vec2};

use super::icons;
use super::theme::{self, pal};
use crate::scanner::ScanHandle;
use crate::util::format;

/// Draws the scan screen. Returns true when the user asked to cancel.
pub fn show(ui: &mut Ui, scan: &ScanHandle, title: &str) -> bool {
    let p = pal(ui);
    let pr = &scan.progress;
    let bytes = pr.bytes.load(Ordering::Relaxed);
    let expected = pr.expected_bytes.load(Ordering::Relaxed);
    let files = pr.files.load(Ordering::Relaxed);
    let dirs = pr.dirs.load(Ordering::Relaxed);
    let errors = pr.errors.load(Ordering::Relaxed);
    let elapsed = scan.started.elapsed().as_secs_f64();
    let frac = if expected > 0 {
        (bytes as f64 / expected as f64).min(0.99) as f32
    } else {
        0.0
    };
    let rate = files as f64 / elapsed.max(0.1);
    let remaining = if frac > 0.02 {
        let total = elapsed / frac as f64;
        Some((total - elapsed).max(0.0))
    } else {
        None
    };
    let current = pr.current.lock().map(|c| c.clone()).unwrap_or_default();
    let mut cancel = false;

    ui.horizontal(|ui| {
        theme::title(ui, title, 22.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if theme::tinted_button(
                ui,
                format!("{}  Cancel scan", icons::X),
                p.danger,
                p.danger_bg,
            )
            .clicked()
            {
                cancel = true;
            }
        });
    });
    ui.add_space(12.0);

    let avail = ui.available_width();
    let ring = 420f32.min(avail * 0.45).max(260.0);
    let wide = avail > ring + 420.0;
    let body = |ui: &mut Ui| {
        // Progress rings
        let (rect, _) = ui.allocate_exact_size(vec2(ring, ring), egui::Sense::hover());
        let painter = ui.painter_at(rect.expand(8.0));
        let c = rect.center();
        let r1 = ring / 2.0 - 12.0;
        painter.circle_stroke(c, r1 + 10.0, egui::Stroke::new(1.0, p.border));
        painter.circle_stroke(c, r1, egui::Stroke::new(16.0, p.track));
        theme::arc(&painter, c, r1, 16.0, 0.0, frac.max(0.004), p.accent, true);
        let r2 = r1 - 34.0;
        painter.circle_stroke(c, r2, egui::Stroke::new(8.0, p.track));
        let t = ui.input(|i| i.time) as f32;
        let spin = (t * 0.35).fract();
        theme::arc(&painter, c, r2, 8.0, spin, spin + 0.22, p.violet, true);
        let r3 = r2 - 24.0;
        painter.circle_stroke(c, r3, egui::Stroke::new(1.0, p.border));
        let dot = theme::arc_points(c, r1 + 10.0, frac, frac + 0.001)[0];
        painter.circle_filled(dot, 4.0, p.accent);
        painter.text(
            c - vec2(0.0, 16.0),
            egui::Align2::CENTER_CENTER,
            format!("{:.0}%", frac * 100.0),
            theme::heading(ring * 0.17),
            p.text,
        );
        let sub = match remaining {
            Some(s) if s >= 1.0 => format!("about {} left", format::duration(s)),
            _ => format!("{} elapsed", format::duration(elapsed)),
        };
        painter.text(
            c + vec2(0.0, ring * 0.09),
            egui::Align2::CENTER_CENTER,
            sub,
            egui::FontId::proportional(14.0),
            p.text_dim,
        );
        painter.text(
            c + vec2(0.0, ring * 0.16),
            egui::Align2::CENTER_CENTER,
            format!("{} files per second", format::count(rate as u64)),
            egui::FontId::proportional(12.0),
            p.success,
        );

        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.set_max_width(560.0);
            let tiles = [
                ("Files", format::count(files)),
                ("Folders", format::count(dirs)),
                ("Size found", format::size(bytes)),
                ("Inaccessible", format::count(errors)),
            ];
            egui::Grid::new("scan_stats")
                .num_columns(2)
                .spacing([14.0, 14.0])
                .show(ui, |ui| {
                    for (i, (label, value)) in tiles.iter().enumerate() {
                        theme::card(ui).show(ui, |ui| {
                            ui.set_width(220.0);
                            theme::stat_tile(ui, label, value);
                        });
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(14.0);
            theme::card(ui).show(ui, |ui| {
                ui.set_width(468.0);
                ui.label(RichText::new("Now reading").size(12.0).color(p.text_muted));
                ui.add(
                    egui::Label::new(RichText::new(current).monospace().size(12.0).color(p.text))
                        .truncate(),
                );
                ui.add_space(6.0);
                theme::bar(ui, 468.0, 6.0, frac, p.accent);
                ui.label(
                    RichText::new(format!(
                        "{} of about {} used on the selected drives",
                        format::size(bytes),
                        format::size(expected)
                    ))
                    .size(12.0)
                    .color(p.text_muted),
                );
            });
        });
    };
    ui.add_space(20.0);
    if wide {
        ui.horizontal_top(|ui| {
            ui.add_space(((avail - ring - 520.0) / 2.0).max(0.0));
            body(ui);
        });
    } else {
        ui.vertical_centered(body);
    }
    ui.ctx().request_repaint_after(Duration::from_millis(33));
    cancel
}
