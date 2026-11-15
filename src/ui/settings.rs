//! Settings page: appearance, updates and about.

use egui::{Color32, RichText, Sense, Ui, vec2};

use crate::system::settings::Settings;
use crate::ui::update::{Event, Phase, Updater};
use crate::ui::{icons, theme};
use crate::update;

pub fn show(
    ui: &mut Ui,
    settings: &mut Settings,
    updater: &mut Updater,
    demo: bool,
    events: &mut Vec<Event>,
) {
    let p = theme::pal(ui);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(760.0);
            theme::title(ui, "Settings", 26.0);
            ui.add_space(16.0);

            section(ui, "Appearance", |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Theme").color(p.text_dim));
                    ui.add_space(40.0);
                    for (dark, icon, label) in
                        [(true, icons::MOON, "Dark"), (false, icons::SUN, "Light")]
                    {
                        let on = settings.dark == dark;
                        if choice(ui, &format!("{icon}  {label}"), on).clicked() && !on {
                            settings.dark = dark;
                            ui.ctx().set_theme(if dark {
                                egui::ThemePreference::Dark
                            } else {
                                egui::ThemePreference::Light
                            });
                            events.push(Event::SaveSettings);
                        }
                    }
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Accent").color(p.text_dim));
                    ui.add_space(34.0);
                    for (name, color) in theme::ACCENTS {
                        let on = settings.accent == name;
                        if accent_swatch(ui, color, on).on_hover_text(name).clicked() && !on {
                            settings.accent = name.to_owned();
                            theme::set_accent(color);
                            theme::apply(ui.ctx());
                            events.push(Event::SaveSettings);
                        }
                    }
                });
            });

            section(ui, "Updates", |ui| {
                ui.label(
                    RichText::new(format!("You have PC Scanner {}.", update::CURRENT))
                        .color(p.text),
                );
                ui.add_space(4.0);
                if updater.store {
                    ui.label(
                        RichText::new(
                            "Installed from the Microsoft Store, which keeps PC Scanner up to \
                             date. PC Scanner itself makes no network requests.",
                        )
                        .color(p.text_muted),
                    );
                    return;
                }
                if theme::checkbox(
                    ui,
                    &mut settings.check_updates,
                    "Check for updates when PC Scanner starts (at most once a day)",
                )
                .changed()
                {
                    settings.update_choice_made = true;
                    updater.ask_consent = false;
                    events.push(Event::SaveSettings);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let busy = updater.busy();
                    if updater.has_update() {
                        let v = updater
                            .release
                            .as_ref()
                            .map(|r| r.version.clone())
                            .unwrap_or_default();
                        if theme::primary_button(
                            ui,
                            icons::DOWNLOAD_SIMPLE,
                            &format!("Get version {v}"),
                        )
                        .clicked()
                        {
                            updater.dialog_open = true;
                        }
                    } else {
                        let r = ui.add_enabled(
                            !busy && !demo,
                            egui::Button::new(format!("{}  Check now", icons::ARROW_CLOCKWISE))
                                .corner_radius(10)
                                .min_size(vec2(0.0, 34.0)),
                        );
                        let r = if demo {
                            r.on_disabled_hover_text("Demo mode makes no network requests.")
                        } else {
                            r
                        };
                        if r.clicked() {
                            updater.start_check(ui.ctx(), true);
                        }
                    }
                    if matches!(updater.phase, Phase::Checking) {
                        ui.spinner();
                    }
                });
                ui.add_space(6.0);
                let failed = matches!(updater.phase, Phase::Failed { .. });
                ui.label(
                    RichText::new(updater.status(settings))
                        .size(13.0)
                        .color(if failed { p.danger } else { p.text_muted }),
                );
                if let Some(v) = settings.skipped_version.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("Version {v} is skipped."))
                                .size(13.0)
                                .color(p.text_muted),
                        );
                        if ui.link("Stop skipping it").clicked() {
                            settings.skipped_version = None;
                            events.push(Event::SaveSettings);
                        }
                    });
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "Updates come from github.com/{}. An update is installed only if its \
                         SHA-256 matches the release's SHA256SUMS.txt and it is signed with the \
                         project's certificate ({}). The check sends no personal data.",
                        update::REPO,
                        update::THUMBPRINT
                    ))
                    .size(12.5)
                    .color(p.text_muted),
                );
            });

            section(ui, "About", |ui| {
                ui.label(
                    RichText::new(format!("PC Scanner {} by Bilal Amjad", update::CURRENT))
                        .color(p.text),
                );
                ui.label(
                    RichText::new("Open source under the MIT licence.")
                        .size(13.0)
                        .color(p.text_muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let base = format!("https://github.com/{}", update::REPO);
                    for (label, url) in [
                        ("Source code", base.clone()),
                        ("Release notes", format!("{base}/releases")),
                        ("Privacy policy", format!("{base}/blob/main/PRIVACY.md")),
                        ("Report a problem", format!("{base}/issues")),
                    ] {
                        if ui
                            .link(RichText::new(label).color(p.accent_text))
                            .on_hover_text(&url)
                            .clicked()
                        {
                            let _ = open::that(&url);
                        }
                        ui.add_space(10.0);
                    }
                });
            });
        });
}

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    theme::card(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        theme::title(ui, title, 18.0);
        ui.add_space(10.0);
        body(ui);
    });
    ui.add_space(14.0);
}

fn choice(ui: &mut Ui, text: &str, on: bool) -> egui::Response {
    let p = theme::pal(ui);
    ui.add(
        egui::Button::new(RichText::new(text).color(if on { p.text } else { p.text_dim }))
            .fill(if on { p.accent_bg } else { p.card })
            .stroke(egui::Stroke::new(1.0, if on { p.accent } else { p.border }))
            .corner_radius(10)
            .min_size(vec2(96.0, 34.0)),
    )
}

fn accent_swatch(ui: &mut Ui, color: Color32, on: bool) -> egui::Response {
    let p = theme::pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, 34.0), Sense::click());
    let c = rect.center();
    if on {
        ui.painter()
            .circle_stroke(c, 16.0, egui::Stroke::new(2.0, p.text));
    } else if resp.hovered() {
        ui.painter()
            .circle_stroke(c, 16.0, egui::Stroke::new(1.0, p.border_hi));
    }
    ui.painter().circle_filled(c, 12.0, color);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
