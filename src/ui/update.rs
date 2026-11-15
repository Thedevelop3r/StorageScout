//! Update state, background work and the "new version" dialog.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};

use egui::{Align, Color32, Layout, RichText, Sense, Ui, vec2};

use crate::system::settings::Settings;
use crate::ui::{icons, theme};
use crate::update::{self, Progress, Release, Verified};
use crate::util::format;

/// Where the update process is.
pub enum Phase {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading(Arc<Progress>),
    /// Verified installer is about to be started.
    Installing(Verified),
    /// `stage` is the check that failed (one of `update::STAGE_*`), if any.
    Failed {
        message: String,
        stage: Option<u8>,
    },
}

enum Msg {
    Checked(Result<Option<Release>, String>),
    Downloaded(Result<Verified, String>, u8),
}

/// What the app should do after the updater ran this frame.
pub enum Event {
    Notify(String),
    SaveSettings,
    /// The installer is running: close PC Scanner so it can be replaced.
    Quit,
}

pub struct Updater {
    pub phase: Phase,
    pub release: Option<Release>,
    pub dialog_open: bool,
    /// Ask once whether update checks are allowed.
    pub ask_consent: bool,
    /// Installed with the installer (portable copies get a download link instead).
    pub installed: bool,
    demo: bool,
    manual: bool,
    rx: Option<Receiver<Msg>>,
}

impl Updater {
    pub fn new(demo: bool) -> Self {
        Self {
            phase: Phase::Idle,
            release: None,
            dialog_open: false,
            ask_consent: false,
            installed: !demo && update::is_installed_copy(),
            demo,
            manual: false,
            rx: None,
        }
    }

    /// Fictional update shown in demo mode (nothing is downloaded).
    pub fn demo_offer(&mut self, open: bool) {
        let prefix = format!(
            "https://github.com/{}/releases/download/v0.3.0/",
            update::REPO
        );
        self.release = Some(Release {
            version: "0.3.0".into(),
            notes: "## What's new\n\
                    * Sunburst explorer and scan history: see what grew since your last scan.\n\
                    * Similar photo finder groups near-identical pictures, not only exact copies.\n\
                    * Quarantine: deleted items can be restored for 30 days.\n\
                    * Faster rescans that only re-read folders that changed."
                .into(),
            page: format!("https://github.com/{}/releases", update::REPO),
            published: Some(chrono::Utc::now() - chrono::Duration::days(2)),
            installer: update::Asset {
                name: "PCScanner-Setup-0.3.0-x64.exe".into(),
                url: format!("{prefix}PCScanner-Setup-0.3.0-x64.exe"),
                size: 6_920_000,
            },
            sums: update::Asset {
                name: "SHA256SUMS.txt".into(),
                url: format!("{prefix}SHA256SUMS.txt"),
                size: 300,
            },
        });
        self.phase = Phase::Available;
        self.dialog_open = open;
    }

    pub fn busy(&self) -> bool {
        matches!(self.phase, Phase::Checking | Phase::Downloading(_))
    }

    /// True when a newer version is known (shown in the top bar).
    pub fn has_update(&self) -> bool {
        self.release.is_some()
    }

    /// Should the automatic start-up check run now?
    pub fn due(settings: &Settings) -> bool {
        let now = chrono::Utc::now().timestamp();
        settings.update_choice_made
            && settings.check_updates
            && settings
                .last_update_check
                .is_none_or(|t| now - t >= 24 * 3600 || t > now)
    }

    pub fn start_check(&mut self, ctx: &egui::Context, manual: bool) {
        if self.demo || self.busy() {
            return;
        }
        self.manual = manual;
        self.phase = Phase::Checking;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Checked(update::check()));
            ctx.request_repaint();
        });
    }

    fn start_download(&mut self, ctx: &egui::Context) {
        let Some(release) = self.release.clone() else {
            return;
        };
        let progress = Arc::new(Progress::default());
        self.phase = Phase::Downloading(progress.clone());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Repaint while downloading so the progress bar moves.
            let ticker = {
                let ctx = ctx.clone();
                let progress = progress.clone();
                std::thread::spawn(move || {
                    while Arc::strong_count(&progress) > 2 {
                        ctx.request_repaint();
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                })
            };
            let result = update::download_and_verify(&release, &progress);
            let stage = progress.stage.load(Ordering::Relaxed);
            let _ = tx.send(Msg::Downloaded(result, stage));
            drop(progress);
            let _ = ticker.join();
            ctx.request_repaint();
        });
    }

    /// Collect results from the background thread.
    pub fn poll(&mut self, settings: &mut Settings) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(msg) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) else {
            return events;
        };
        self.rx = None;
        match msg {
            Msg::Checked(Ok(found)) => {
                settings.last_update_check = Some(chrono::Utc::now().timestamp());
                events.push(Event::SaveSettings);
                match found {
                    Some(r) => {
                        let skipped = settings.skipped_version.as_deref() == Some(&r.version);
                        if self.manual || !skipped {
                            self.dialog_open = true;
                        }
                        self.release = Some(r);
                        self.phase = Phase::Available;
                    }
                    None => {
                        self.release = None;
                        self.phase = Phase::UpToDate;
                        if self.manual {
                            events.push(Event::Notify(format!(
                                "PC Scanner {} is the latest version.",
                                update::CURRENT
                            )));
                        }
                    }
                }
            }
            Msg::Checked(Err(e)) => {
                if self.manual {
                    events.push(Event::Notify(e.clone()));
                }
                self.phase = Phase::Failed {
                    message: e,
                    stage: None,
                };
            }
            Msg::Downloaded(Ok(v), _) => match update::launch_installer(&v.path) {
                Ok(()) => {
                    self.phase = Phase::Installing(v);
                    events.push(Event::Quit);
                }
                Err(e) => {
                    self.phase = Phase::Failed {
                        message: e,
                        stage: None,
                    }
                }
            },
            Msg::Downloaded(Err(e), stage) => {
                self.phase = Phase::Failed {
                    message: e,
                    stage: Some(stage),
                };
            }
        }
        events
    }

    /// One-line status for the Settings page.
    pub fn status(&self, settings: &Settings) -> String {
        match &self.phase {
            Phase::Checking => "Checking GitHub for a new version…".into(),
            Phase::Downloading(_) => "Downloading the update…".into(),
            Phase::Installing(_) => "Installing the update…".into(),
            Phase::Available => match &self.release {
                Some(r) => format!("Version {} is available.", r.version),
                None => String::new(),
            },
            Phase::Failed { message, .. } => message.clone(),
            Phase::UpToDate | Phase::Idle => match settings.last_update_check {
                Some(t) => format!(
                    "You have the latest version. Last checked {}.",
                    format::age(t)
                ),
                None => "Not checked yet.".into(),
            },
        }
    }

    /// The update dialog. Returns events for the app.
    pub fn dialog(&mut self, ui: &mut Ui, settings: &mut Settings) -> Vec<Event> {
        let mut events = Vec::new();
        if !self.dialog_open {
            return events;
        }
        let Some(release) = self.release.clone() else {
            self.dialog_open = false;
            return events;
        };
        let p = theme::pal(ui);
        let ctx = ui.ctx().clone();
        let modal = egui::Modal::new(egui::Id::unique("update_dialog"))
            .frame(
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border_hi))
                    .corner_radius(22)
                    .inner_margin(32),
            )
            .show(&ctx, |ui| {
                ui.set_width(576.0);
                let mut close = false;
                // Header
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(52.0, 52.0), Sense::hover());
                    ui.painter().rect_filled(rect, 14.0, p.success_bg);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icons::DOWNLOAD_SIMPLE,
                        egui::FontId::proportional(24.0),
                        p.success,
                    );
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.horizontal(|ui| {
                            theme::title(
                                ui,
                                &format!("PC Scanner {} is ready", release.version),
                                24.0,
                            );
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(icons::X).size(15.0).color(p.text_dim),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::new(1.0, p.border))
                                        .corner_radius(10)
                                        .min_size(vec2(34.0, 34.0)),
                                    )
                                    .on_hover_text("Remind me later")
                                    .clicked()
                                {
                                    close = true;
                                }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            theme::pill(
                                ui,
                                format!("You have {}", update::CURRENT),
                                p.text_dim,
                                p.track,
                            );
                            ui.label(RichText::new(icons::ARROW_RIGHT).color(p.text_muted));
                            theme::pill(
                                ui,
                                format!(
                                    "{} · {}",
                                    release.version,
                                    format::size(release.installer.size)
                                ),
                                p.success,
                                p.success_bg,
                            );
                            if let Some(t) = release.published {
                                ui.label(
                                    RichText::new(format!(
                                        "Published {} on GitHub",
                                        format::age(t.timestamp())
                                    ))
                                    .size(13.0)
                                    .color(p.text_muted),
                                );
                            }
                        });
                    });
                });
                ui.add_space(18.0);

                // What's new
                ui.label(
                    RichText::new("WHAT'S NEW")
                        .size(11.5)
                        .color(p.text_muted)
                        .extra_letter_spacing(0.8),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(170.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let lines = update::notes_lines(&release.notes);
                        if lines.is_empty() {
                            ui.label(RichText::new("No release notes.").color(p.text_muted));
                        }
                        for (heading, bullet, text) in lines.into_iter().take(40) {
                            if heading {
                                // The section titles from GitHub add little here.
                                continue;
                            }
                            ui.horizontal_wrapped(|ui| {
                                if bullet {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                                    ui.painter().circle_filled(r.center(), 3.0, p.accent);
                                    ui.add_space(4.0);
                                }
                                ui.label(RichText::new(text).size(14.0).color(p.text));
                            });
                        }
                    });
                if ui
                    .link(RichText::new("Full release notes on GitHub").color(p.accent_text))
                    .clicked()
                {
                    let _ = open::that(&release.page);
                }
                ui.add_space(14.0);

                // Checks
                theme::tile(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Checked before installing").font(theme::bold(14.0)));
                    ui.add_space(6.0);
                    let marks = self.marks();
                    let short = format!("{}…{}", &update::THUMBPRINT[..4], &update::THUMBPRINT[36..]);
                    check_row(ui, marks[0], "Downloaded from the official GitHub release", update::REPO);
                    check_row(ui, marks[1], "SHA-256 matches SHA256SUMS.txt", match marks[1] {
                        Mark::Pending => "waiting for download",
                        Mark::Working => "checking",
                        Mark::Done => "matches",
                        Mark::Failed => "does not match",
                    });
                    check_row(ui, marks[2], "Signed by Bilal Amjad (PC Scanner)", &short);
                    check_row(ui, Mark::Done, "Your settings and scan history are kept", "");
                    match &self.phase {
                        Phase::Downloading(pr) => {
                            let done = pr.done.load(Ordering::Relaxed);
                            let total = pr.total.load(Ordering::Relaxed).max(1);
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Downloading installer")
                                        .size(12.5)
                                        .color(p.text_muted),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} of {}",
                                            format::size(done),
                                            format::size(total)
                                        ))
                                        .size(12.5)
                                        .color(p.text_muted),
                                    );
                                });
                            });
                            let w = ui.available_width();
                            theme::bar(ui, w, 6.0, done as f32 / total as f32, p.success);
                        }
                        Phase::Failed { message, stage } if stage.is_some() => {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!("{}  {message} Nothing was installed.", icons::WARNING))
                                    .color(p.danger),
                            );
                        }
                        Phase::Installing(v) => {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(format!(
                                    "Verified: signed by {}, SHA-256 {}…. Installing; PC Scanner will restart.",
                                    v.signer,
                                    &v.sha256[..12]
                                ))
                                .color(p.success),
                            );
                        }
                        _ => {}
                    }
                });
                ui.add_space(14.0);

                if theme::checkbox(ui, &mut settings.check_updates, "Check for updates when PC Scanner starts")
                    .changed()
                {
                    settings.update_choice_made = true;
                    events.push(Event::SaveSettings);
                }
                ui.add_space(14.0);

                // Buttons
                ui.horizontal(|ui| {
                    let busy = matches!(self.phase, Phase::Downloading(_) | Phase::Installing(_));
                    if ui
                        .add_enabled(
                            !busy,
                            egui::Button::new(RichText::new("Skip this version").color(p.text_dim))
                                .frame(false),
                        )
                        .clicked()
                    {
                        settings.skipped_version = Some(release.version.clone());
                        events.push(Event::SaveSettings);
                        events.push(Event::Notify(format!(
                            "Version {} skipped. You can still install it from Settings.",
                            release.version
                        )));
                        close = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !self.installed {
                            let r = theme::primary_button(
                                ui,
                                icons::ARROW_SQUARE_OUT,
                                "Open download page",
                            )
                            .on_hover_text(if self.demo {
                                "Demo mode: this would install the update after the checks above."
                            } else {
                                "This is a portable copy, so it cannot replace itself. \
                                 Download the new version from GitHub."
                            });
                            if r.clicked() && !self.demo {
                                let _ = open::that(&release.page);
                                close = true;
                            }
                        } else {
                            match &self.phase {
                                Phase::Downloading(pr) => {
                                    if theme::secondary_button(ui, "Cancel").clicked() {
                                        pr.cancel.store(true, Ordering::Relaxed);
                                    }
                                }
                                Phase::Installing(_) => {}
                                _ => {
                                    let retry = matches!(self.phase, Phase::Failed { .. });
                                    let label =
                                        if retry { "Try again" } else { "Install and restart" };
                                    if theme::primary_button(ui, icons::DOWNLOAD_SIMPLE, label)
                                        .clicked()
                                    {
                                        self.start_download(ui.ctx());
                                    }
                                }
                            }
                        }
                        if !busy && theme::secondary_button(ui, "Remind me later").clicked() {
                            close = true;
                        }
                    });
                });
                close
            });
        if modal.inner || (modal.should_close() && !self.busy()) {
            self.dialog_open = false;
        }
        events
    }

    /// First-start question: may PC Scanner check GitHub for updates?
    pub fn consent_dialog(&mut self, ui: &mut Ui, settings: &mut Settings) -> Vec<Event> {
        let mut events = Vec::new();
        if !self.ask_consent || self.dialog_open {
            return events;
        }
        let p = theme::pal(ui);
        let ctx = ui.ctx().clone();
        let mut answer: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("update_consent"))
            .frame(
                egui::Frame::new()
                    .fill(p.card)
                    .stroke(egui::Stroke::new(1.0, p.border_hi))
                    .corner_radius(22)
                    .inner_margin(28),
            )
            .show(&ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                    ui.painter().rect_filled(rect, 12.0, p.accent_bg);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        icons::ARROW_CLOCKWISE,
                        egui::FontId::proportional(21.0),
                        p.accent_text,
                    );
                    ui.add_space(4.0);
                    theme::title(ui, "Keep PC Scanner up to date?", 20.0);
                });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "PC Scanner can check GitHub for a new version when it starts, at most \
                         once a day. Only the release list is downloaded: nothing about you or \
                         this computer is sent. You always decide whether to install an update.",
                    )
                    .color(p.text_dim),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new("You can change this later in Settings.")
                        .size(13.0)
                        .color(p.text_muted),
                );
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::primary_button(ui, icons::CHECK, "Check for updates").clicked() {
                            answer = Some(true);
                        }
                        if theme::secondary_button(ui, "No thanks").clicked() {
                            answer = Some(false);
                        }
                    });
                });
            });
        if let Some(yes) = answer {
            self.ask_consent = false;
            settings.check_updates = yes;
            settings.update_choice_made = true;
            events.push(Event::SaveSettings);
            if yes {
                self.start_check(&ctx, false);
            }
        }
        events
    }

    /// Progress of the three checks: download source, SHA-256, signature.
    fn marks(&self) -> [Mark; 3] {
        use update::{STAGE_DOWNLOAD, STAGE_HASH, STAGE_SIGNATURE};
        let by_stage = |stage: u8, failed: bool| {
            let at = |s: u8| {
                if stage > s {
                    Mark::Done
                } else if stage == s {
                    if failed { Mark::Failed } else { Mark::Working }
                } else {
                    Mark::Pending
                }
            };
            [at(STAGE_DOWNLOAD), at(STAGE_HASH), at(STAGE_SIGNATURE)]
        };
        match &self.phase {
            Phase::Downloading(pr) => by_stage(pr.stage.load(Ordering::Relaxed), false),
            Phase::Failed { stage: Some(s), .. } => by_stage(*s, true),
            Phase::Installing(_) => [Mark::Done; 3],
            _ => [Mark::Pending; 3],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Pending,
    Working,
    Done,
    Failed,
}

fn check_row(ui: &mut Ui, mark: Mark, text: &str, detail: &str) {
    let p = theme::pal(ui);
    ui.horizontal(|ui| {
        ui.set_min_height(30.0);
        let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        let c = rect.center();
        let (bg, fg, glyph) = match mark {
            Mark::Done => (p.success_bg, p.success, icons::CHECK),
            Mark::Failed => (p.danger_bg, p.danger, icons::X),
            Mark::Working => (p.accent_bg, p.accent_text, ""),
            Mark::Pending => (p.track, p.text_muted, icons::HOURGLASS),
        };
        ui.painter().circle_filled(c, 11.0, bg);
        if mark == Mark::Working {
            let t = ui.input(|i| i.time) as f32;
            let f = (t * 1.2).fract();
            theme::arc(ui.painter(), c, 6.5, 2.0, f, f + 0.3, fg, true);
            ui.ctx().request_repaint();
        } else if !glyph.is_empty() {
            ui.painter().text(
                c,
                egui::Align2::CENTER_CENTER,
                glyph,
                egui::FontId::proportional(12.0),
                fg,
            );
        }
        ui.add_space(4.0);
        ui.label(RichText::new(text).size(14.0).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(detail)
                    .font(egui::FontId::monospace(11.5))
                    .color(p.text_muted),
            );
        });
    });
}
