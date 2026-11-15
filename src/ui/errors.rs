//! Paths that could not be read during the scan.

use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};

use super::{ScanData, UiAction};
use crate::util::format;

#[derive(Default)]
pub struct ErrorsState {
    pub filter: String,
}

pub fn show(ui: &mut Ui, st: &mut ErrorsState, data: &ScanData, actions: &mut Vec<UiAction>) {
    ui.heading("Inaccessible locations");
    ui.label(RichText::new(
        "These folders could not be read (usually “Access is denied”). Their size is not included. \
         Run StorageScout as administrator to include them.",
    ).weak());
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(
            egui::TextEdit::singleline(&mut st.filter)
                .hint_text("Filter")
                .desired_width(220.0),
        );
        ui.label(format!(
            "{} entries",
            format::count(data.errors.len() as u64)
        ));
    });
    ui.separator();
    let f = st.filter.to_lowercase();
    let rows: Vec<_> = data
        .errors
        .iter()
        .filter(|e| {
            f.is_empty()
                || e.path.to_string_lossy().to_lowercase().contains(&f)
                || e.message.to_lowercase().contains(&f)
        })
        .collect();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(egui::Sense::click())
        .column(Column::initial(520.0).clip(true))
        .column(Column::remainder())
        .header(20.0, |mut h| {
            h.col(|ui| {
                ui.strong("Path");
            });
            h.col(|ui| {
                ui.strong("Reason");
            });
        })
        .body(|body| {
            body.rows(20.0, rows.len(), |mut row| {
                let e = rows[row.index()];
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(e.path.to_string_lossy())
                            .truncate()
                            .selectable(false),
                    );
                });
                row.col(|ui| {
                    ui.label(RichText::new(&e.message).weak());
                });
                let resp = row.response();
                resp.context_menu(|ui| {
                    if ui.button("🔎 Show in file manager").clicked() {
                        actions.push(UiAction::Reveal(e.path.clone()));
                        ui.close();
                    }
                    if ui.button("📋 Copy path").clicked() {
                        actions.push(UiAction::CopyText(e.path.to_string_lossy().into_owned()));
                        ui.close();
                    }
                });
            });
        });
}
