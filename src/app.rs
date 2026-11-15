//! Application state, layout and action handling.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};

use crate::analysis::{self, Analysis};
use crate::model::NodeId;
use crate::scanner::ScanHandle;
use crate::system::actions::{self, DeleteJob, DeleteMode, DeleteTarget};
use crate::system::drives::{self, DriveInfo};
use crate::system::programs::{self, Program};
use crate::ui::{self, Ctx, ScanData, TableOptions, UiAction};
use crate::util::{export, format};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Overview,
    Explorer,
    Treemap,
    Categories,
    Extensions,
    LargestFiles,
    LargestFolders,
    OldFiles,
    EmptyFolders,
    Duplicates,
    Programs,
    Cleanup,
    Search,
    Selection,
    Errors,
}

impl View {
    const ALL: [View; 15] = [
        View::Overview,
        View::Explorer,
        View::Treemap,
        View::Categories,
        View::Extensions,
        View::LargestFiles,
        View::LargestFolders,
        View::OldFiles,
        View::EmptyFolders,
        View::Duplicates,
        View::Programs,
        View::Cleanup,
        View::Search,
        View::Selection,
        View::Errors,
    ];

    /// Parse a view name such as "treemap" or "largest-files".
    fn from_name(name: &str) -> Option<View> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        View::ALL.into_iter().find(|v| {
            let dbg = format!("{v:?}").to_lowercase();
            let label: String = v
                .label()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
                .to_lowercase();
            dbg == n || label == n
        })
    }

    fn label(self) -> &'static str {
        match self {
            View::Overview => "🏠 Overview",
            View::Explorer => "📂 Explorer",
            View::Treemap => "▦ Treemap",
            View::Categories => "📊 Groups",
            View::Extensions => "🏷 File types",
            View::LargestFiles => "📄 Largest files",
            View::LargestFolders => "📁 Largest folders",
            View::OldFiles => "⌛ Old files",
            View::EmptyFolders => "📂 Empty folders",
            View::Duplicates => "👯 Duplicates",
            View::Programs => "⚙ Programs",
            View::Cleanup => "✨ Cleanup",
            View::Search => "🔍 Search",
            View::Selection => "☑ Selection",
            View::Errors => "⚠ Inaccessible",
        }
    }
}

const NAV: [(&str, &[View]); 4] = [
    (
        "",
        &[View::Overview, View::Explorer, View::Treemap, View::Search],
    ),
    (
        "GROUPS",
        &[View::Categories, View::Extensions, View::Programs],
    ),
    (
        "FIND SPACE",
        &[
            View::LargestFiles,
            View::LargestFolders,
            View::OldFiles,
            View::Duplicates,
            View::EmptyFolders,
        ],
    ),
    ("ACT", &[View::Cleanup, View::Selection, View::Errors]),
];

struct ConfirmDelete {
    targets: Vec<DeleteTarget>,
    protected: Vec<(PathBuf, &'static str)>,
    mode: DeleteMode,
    total: u64,
    contents_only: bool,
}

pub struct PcScannerApp {
    view: View,
    drives: Vec<DriveInfo>,
    drive_checked: Vec<bool>,
    custom_roots: Vec<PathBuf>,
    scan: Option<ScanHandle>,
    data: Option<ScanData>,
    programs: Option<Vec<Program>>,
    programs_rx: Option<Receiver<Vec<Program>>>,
    selection: BTreeSet<NodeId>,
    confirm: Option<ConfirmDelete>,
    delete_job: Option<DeleteJob>,
    message: Option<(String, Instant)>,
    dark: bool,
    /// Showing built-in fictional data; nothing on the real disk may be touched.
    demo: bool,

    explorer: ui::explorer::ExplorerState,
    treemap: ui::treemap::TreemapState,
    categories: ui::categories::CategoriesState,
    extensions: ui::extensions::ExtensionsState,
    largest_files: ui::files::FileListState,
    largest_folders: ui::files::FileListState,
    old_files: ui::files::FileListState,
    empty_folders: ui::files::FileListState,
    duplicates: ui::duplicates::DuplicatesState,
    programs_view: ui::programs::ProgramsState,
    cleanup: ui::cleanup::CleanupState,
    search: ui::search::SearchState,
    errors: ui::errors::ErrorsState,
}

impl PcScannerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        start_paths: Vec<PathBuf>,
        start_view: Option<&str>,
        demo: bool,
    ) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        cc.egui_ctx.global_style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(8.0, 5.0);
            s.spacing.button_padding = egui::vec2(8.0, 3.0);
        });
        if demo {
            return Self::new_demo(start_view);
        }
        let drives = drives::list_drives();
        // Pre-select fixed drives.
        let drive_checked = drives.iter().map(|d| !d.removable).collect();

        let (tx, rx) = mpsc::channel();
        let ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(programs::list_installed());
            ctx.request_repaint();
        });

        let mut app = Self {
            view: View::Overview,
            drives,
            drive_checked,
            custom_roots: Vec::new(),
            scan: None,
            data: None,
            programs: None,
            programs_rx: Some(rx),
            demo: false,
            selection: BTreeSet::new(),
            confirm: None,
            delete_job: None,
            message: None,
            dark: true,
            explorer: Default::default(),
            treemap: Default::default(),
            categories: Default::default(),
            extensions: Default::default(),
            largest_files: ui::files::FileListState::new(ui::files::ListKind::LargestFiles),
            largest_folders: ui::files::FileListState::new(ui::files::ListKind::LargestFolders),
            old_files: ui::files::FileListState::new(ui::files::ListKind::OldFiles),
            empty_folders: ui::files::FileListState::new(ui::files::ListKind::EmptyFolders),
            duplicates: Default::default(),
            programs_view: Default::default(),
            cleanup: Default::default(),
            search: Default::default(),
            errors: Default::default(),
        };
        if !start_paths.is_empty() {
            app.drive_checked.iter_mut().for_each(|c| *c = false);
            for p in start_paths {
                // A drive root ticks that drive; anything else is a custom folder.
                match app.drives.iter().position(|d| same_path(&d.mount, &p)) {
                    Some(i) => app.drive_checked[i] = true,
                    None => app.custom_roots.push(p),
                }
            }
            app.start_scan(&cc.egui_ctx);
        }
        if let Some(v) = start_view.and_then(View::from_name) {
            app.view = v;
        }
        app
    }

    /// App filled with the fictional demo computer (no disk or registry access).
    fn new_demo(start_view: Option<&str>) -> Self {
        let (drives, data, dups) = crate::demo::scan_data();
        let drive_checked = vec![true; drives.len()];
        let generation = data.generation;
        let c_root = data.tree.roots[0];
        let home = data.tree.find_path(std::path::Path::new(r"C:\Users\Alex"));
        let mut app = Self {
            view: View::Overview,
            drives,
            drive_checked,
            custom_roots: Vec::new(),
            scan: None,
            data: Some(data),
            programs: Some(crate::demo::programs()),
            programs_rx: None,
            selection: BTreeSet::new(),
            confirm: None,
            delete_job: None,
            message: Some((
                "Demo mode: fictional data. Opening and deleting files is disabled.".into(),
                Instant::now(),
            )),
            dark: true,
            demo: true,
            explorer: Default::default(),
            treemap: Default::default(),
            categories: Default::default(),
            extensions: Default::default(),
            largest_files: ui::files::FileListState::new(ui::files::ListKind::LargestFiles),
            largest_folders: ui::files::FileListState::new(ui::files::ListKind::LargestFolders),
            old_files: ui::files::FileListState::new(ui::files::ListKind::OldFiles),
            empty_folders: ui::files::FileListState::new(ui::files::ListKind::EmptyFolders),
            duplicates: Default::default(),
            programs_view: Default::default(),
            cleanup: Default::default(),
            search: Default::default(),
            errors: Default::default(),
        };
        app.duplicates.groups = Some(dups);
        app.duplicates.generation = generation;
        app.treemap.current = Some(c_root);
        app.explorer.current = home;
        if let Some(v) = start_view.and_then(View::from_name) {
            app.view = v;
        }
        app
    }

    /// Re-read drive sizes, keeping the user's check marks.
    fn refresh_drives(&mut self) {
        if self.demo {
            return;
        }
        let new = drives::list_drives();
        let checked = new
            .iter()
            .map(|d| {
                self.drives
                    .iter()
                    .position(|o| o.mount == d.mount)
                    .map(|i| self.drive_checked[i])
                    .unwrap_or(false)
            })
            .collect();
        self.drives = new;
        self.drive_checked = checked;
    }

    fn notify(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), Instant::now()));
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.demo {
            self.notify("Scanning is disabled in demo mode. Start PC Scanner without --demo to scan this PC.");
            return;
        }
        let mut roots: Vec<PathBuf> = Vec::new();
        let mut expected = 0;
        for (d, &c) in self.drives.iter().zip(&self.drive_checked) {
            if c {
                roots.push(d.mount.clone());
                expected += d.used();
            }
        }
        for r in &self.custom_roots {
            if !roots.iter().any(|x| r.starts_with(x)) {
                roots.push(r.clone());
            }
        }
        if roots.is_empty() {
            self.notify("Select at least one drive or folder to scan.");
            return;
        }
        // Free memory of the previous scan first.
        self.data = None;
        self.selection.clear();
        self.explorer = Default::default();
        self.treemap = Default::default();
        self.duplicates = Default::default();
        self.search = Default::default();
        self.scan = Some(ScanHandle::start(roots, expected, ctx.clone()));
    }

    fn poll_background(&mut self) {
        if let Some(rx) = &self.programs_rx
            && let Ok(p) = rx.try_recv()
        {
            self.programs = Some(p);
            self.programs_rx = None;
        }
        if let Some(scan) = &self.scan
            && let Some(r) = scan.poll()
        {
            let generation = self.data.as_ref().map(|d| d.generation + 1).unwrap_or(1);
            let msg = format!(
                "Scan {} in {} — {} in {} files.",
                if r.cancelled { "cancelled" } else { "finished" },
                format::duration(r.elapsed.as_secs_f64()),
                format::size(r.analysis.total_size),
                format::count(r.analysis.total_files),
            );
            self.data = Some(ScanData {
                tree: r.tree,
                analysis: r.analysis,
                cleanup: r.cleanup,
                errors: r.errors,
                elapsed_secs: r.elapsed.as_secs_f64(),
                cancelled: r.cancelled,
                roots: r.roots,
                generation,
            });
            self.scan = None;
            self.refresh_drives();
            self.notify(msg);
        }
        if let Some(job) = &self.delete_job
            && let Some(outcomes) = job.poll()
        {
            self.delete_job = None;
            self.apply_delete_outcomes(outcomes);
        }
    }

    fn apply_delete_outcomes(&mut self, outcomes: Vec<actions::DeleteOutcome>) {
        let Some(data) = &mut self.data else { return };
        let mut ok = 0;
        let mut failed = 0;
        let mut freed = 0u64;
        let mut first_err = None;
        for o in outcomes {
            let before = data.tree.get(o.target.id).size;
            if o.target.contents_only {
                let kids: Vec<NodeId> = data.tree.children(o.target.id).collect();
                for k in kids {
                    if !o
                        .kept_children
                        .iter()
                        .any(|n| **n == *data.tree.get(k).name)
                    {
                        data.tree.mark_deleted(k);
                    }
                }
                freed += before - data.tree.get(o.target.id).size;
            } else if o.result.is_ok() {
                data.tree.mark_deleted(o.target.id);
                freed += before;
            }
            match o.result {
                Ok(()) => ok += 1,
                Err(e) => {
                    failed += 1;
                    if first_err.is_none() {
                        first_err = Some(format!("{}: {e}", o.target.path.display()));
                    }
                }
            }
            self.selection.remove(&o.target.id);
        }
        let tree = &data.tree;
        self.selection.retain(|&id| !tree.is_gone(id));
        data.analysis = Analysis::build(&data.tree);
        data.cleanup = analysis::cleanup::suggestions(&data.tree);
        data.generation += 1;
        self.refresh_drives();
        let mut msg = format!("Cleaned {ok} item(s), freed {}.", format::size(freed));
        if failed > 0 {
            msg.push_str(&format!(
                " {failed} failed — {}",
                first_err.unwrap_or_default()
            ));
        }
        self.notify(msg);
    }

    fn request_delete(&mut self, ids: Vec<NodeId>, contents_only: bool) {
        let Some(data) = &self.data else { return };
        let tree = &data.tree;
        // Drop items already covered by a selected ancestor.
        let set: BTreeSet<NodeId> = ids
            .iter()
            .copied()
            .filter(|&id| !tree.is_gone(id))
            .collect();
        let mut targets = Vec::new();
        let mut protected = Vec::new();
        for &id in &set {
            let mut p = tree.get(id).parent;
            let mut covered = false;
            while p != crate::model::tree::NO_PARENT {
                if set.contains(&p) {
                    covered = true;
                    break;
                }
                p = tree.get(p).parent;
            }
            if covered {
                continue;
            }
            let path = tree.path(id);
            let contents = contents_only && tree.get(id).is_dir;
            if !contents && let Some(reason) = actions::protection_reason(&path) {
                protected.push((path, reason));
                continue;
            }
            targets.push(DeleteTarget {
                id,
                path,
                contents_only: contents,
                size: tree.get(id).size,
            });
        }
        let total = targets.iter().map(|t| t.size).sum();
        if targets.is_empty() && protected.is_empty() {
            return;
        }
        self.confirm = Some(ConfirmDelete {
            targets,
            protected,
            mode: DeleteMode::RecycleBin,
            total,
            contents_only,
        });
    }

    fn handle_actions(&mut self, ctx: &egui::Context, list: Vec<UiAction>) {
        for a in list {
            // Demo paths are fictional and may collide with real ones: never touch the disk.
            if self.demo
                && matches!(
                    a,
                    UiAction::Open(_)
                        | UiAction::Reveal(_)
                        | UiAction::Delete(_)
                        | UiAction::DeleteContents(_)
                )
            {
                self.notify("Demo mode: opening and deleting files is disabled.");
                continue;
            }
            match a {
                UiAction::Open(p) => {
                    if let Err(e) = actions::open_path(&p) {
                        self.notify(format!("Could not open {}: {e}", p.display()));
                    }
                }
                UiAction::Reveal(p) => {
                    if let Err(e) = actions::reveal(&p) {
                        self.notify(format!("Could not show {}: {e}", p.display()));
                    }
                }
                UiAction::CopyText(t) => {
                    ctx.copy_text(t);
                    self.notify("Copied to clipboard.");
                }
                UiAction::ToggleSelect(id) => {
                    if !self.selection.remove(&id) {
                        self.selection.insert(id);
                    }
                }
                UiAction::Select(ids) => {
                    let n = ids.len();
                    self.selection.extend(ids);
                    self.notify(format!(
                        "Added {} item(s) to the selection.",
                        format::count(n as u64)
                    ));
                }
                UiAction::Unselect(ids) => {
                    for id in ids {
                        self.selection.remove(&id);
                    }
                }
                UiAction::Delete(ids) => self.request_delete(ids, false),
                UiAction::DeleteContents(ids) => self.request_delete(ids, true),
                UiAction::ShowInExplorer(id) => {
                    if let Some(d) = &self.data {
                        let target = if d.tree.get(id).is_dir {
                            id
                        } else {
                            d.tree.get(id).parent
                        };
                        self.explorer.navigate(Some(target));
                        self.view = View::Explorer;
                    }
                }
                UiAction::ShowInTreemap(id) => {
                    self.treemap.current = Some(id);
                    self.view = View::Treemap;
                }
                UiAction::Notify(m) => self.notify(m),
            }
        }
    }

    // ------------------------------------------------------------------ panels

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("🖥 PC Scanner").heading().strong());
            if self.demo {
                ui.label(
                    RichText::new(" DEMO ")
                        .strong()
                        .color(Color32::BLACK)
                        .background_color(Color32::from_rgb(0xF2, 0xB6, 0x3C)),
                )
                .on_hover_text("Fictional sample data. Start without --demo to scan this PC.");
            }
            ui.separator();
            let scanning = self.scan.is_some();
            ui.add_enabled_ui(!scanning, |ui| {
                for (i, d) in self.drives.iter().enumerate() {
                    let label = format!("💽 {}  {}", d.mount.display(), format::size(d.used()));
                    ui.checkbox(&mut self.drive_checked[i], label)
                        .on_hover_text(format!(
                            "{}\n{} free of {}",
                            d.display_name(),
                            format::size(d.available),
                            format::size(d.total)
                        ));
                }
                let mut remove = None;
                for (i, r) in self.custom_roots.iter().enumerate() {
                    if ui
                        .button(format!("📁 {}  ✖", r.display()))
                        .on_hover_text("Remove this folder")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    self.custom_roots.remove(i);
                }
                if ui
                    .button("➕ Folder…")
                    .on_hover_text("Scan a specific folder")
                    .clicked()
                    && let Some(dir) = rfd::FileDialog::new().pick_folder()
                {
                    self.custom_roots.push(dir);
                }
            });
            ui.separator();
            if let Some(scan) = &self.scan {
                if ui.button(RichText::new("⏹ Cancel").strong()).clicked() {
                    scan.cancel();
                }
            } else if ui
                .button(RichText::new("▶ Scan").strong().size(15.0))
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.start_scan(&ctx);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let theme_icon = if self.dark { "☀" } else { "🌙" };
                if ui
                    .button(theme_icon)
                    .on_hover_text("Toggle light / dark theme")
                    .clicked()
                {
                    self.dark = !self.dark;
                    ui.ctx().set_theme(if self.dark {
                        egui::ThemePreference::Dark
                    } else {
                        egui::ThemePreference::Light
                    });
                }
                if ui
                    .button("🔄")
                    .on_hover_text("Refresh drive list")
                    .clicked()
                {
                    self.refresh_drives();
                }
                ui.menu_button("💾 Export", |ui| self.export_menu(ui));
            });
        });

        if let Some(scan) = &self.scan {
            let p = &scan.progress;
            let bytes = p.bytes.load(Ordering::Relaxed);
            let expected = p.expected_bytes.load(Ordering::Relaxed);
            let files = p.files.load(Ordering::Relaxed);
            let dirs = p.dirs.load(Ordering::Relaxed);
            let errors = p.errors.load(Ordering::Relaxed);
            let elapsed = scan.started.elapsed().as_secs_f64();
            let frac = if expected > 0 {
                (bytes as f32 / expected as f32).min(0.99)
            } else {
                0.0
            };
            ui.add(egui::ProgressBar::new(frac).animate(true).text(format!(
                "{}  ·  {} files  ·  {} folders  ·  {} inaccessible  ·  {}  ·  {:.0} files/s",
                format::size(bytes),
                format::count(files),
                format::count(dirs),
                format::count(errors),
                format::duration(elapsed),
                files as f64 / elapsed.max(0.1)
            )));
            let current = p.current.lock().map(|c| c.clone()).unwrap_or_default();
            ui.add(egui::Label::new(RichText::new(current).weak().small()).truncate());
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        ui.add_space(2.0);
    }

    fn export_menu(&mut self, ui: &mut Ui) {
        let has_data = self.data.is_some();
        let mut result: Option<Result<String, String>> = None;
        if ui
            .add_enabled(has_data, egui::Button::new("Largest files (CSV)…"))
            .clicked()
        {
            if let (Some(d), Some(p)) = (&self.data, save_dialog("largest_files.csv", "csv")) {
                result = Some(
                    export::export_files_csv(d, &p, 100_000)
                        .map(|n| format!("Exported {n} files to {}", p.display())),
                );
            }
            ui.close();
        }
        if ui
            .add_enabled(has_data, egui::Button::new("Largest folders (CSV)…"))
            .clicked()
        {
            if let (Some(d), Some(p)) = (&self.data, save_dialog("largest_folders.csv", "csv")) {
                result = Some(
                    export::export_folders_csv(d, &p, 100_000)
                        .map(|n| format!("Exported {n} folders to {}", p.display())),
                );
            }
            ui.close();
        }
        if ui
            .add_enabled(
                self.programs.is_some(),
                egui::Button::new("Installed programs (CSV)…"),
            )
            .clicked()
        {
            if let (Some(progs), Some(p)) = (&self.programs, save_dialog("programs.csv", "csv")) {
                result = Some(
                    export::export_programs_csv(progs, &p)
                        .map(|n| format!("Exported {n} programs to {}", p.display())),
                );
            }
            ui.close();
        }
        if ui
            .add_enabled(has_data, egui::Button::new("Scan summary (JSON)…"))
            .clicked()
        {
            if let (Some(d), Some(p)) = (&self.data, save_dialog("pcscanner_summary.json", "json"))
            {
                result = Some(
                    export::export_summary_json(d, &p)
                        .map(|_| format!("Saved summary to {}", p.display())),
                );
            }
            ui.close();
        }
        match result {
            Some(Ok(m)) => self.notify(m),
            Some(Err(e)) => self.notify(format!("Export failed: {e}")),
            None => {}
        }
    }

    fn nav_panel(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        let data = self.data.as_ref();
        for (title, views) in NAV {
            if !title.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new(title).small().weak());
            }
            for &v in views {
                let badge = match (v, data) {
                    (View::Cleanup, Some(d)) => {
                        Some(format::size(d.cleanup.iter().map(|c| c.size).sum()))
                    }
                    (View::Selection, _) if !self.selection.is_empty() => {
                        Some(format::count(self.selection.len() as u64))
                    }
                    (View::Errors, Some(d)) if !d.errors.is_empty() => {
                        Some(format::count(d.errors.len() as u64))
                    }
                    (View::Programs, _) => self.programs.as_ref().map(|p| p.len().to_string()),
                    _ => None,
                };
                let selected = self.view == v;
                let resp = ui.add_sized(
                    [ui.available_width(), 26.0],
                    egui::Button::selectable(selected, v.label()).right_text(
                        badge
                            .map(|b| RichText::new(b).small().weak())
                            .unwrap_or_default(),
                    ),
                );
                if resp.clicked() {
                    self.view = v;
                }
            }
        }
    }

    fn bottom_bar(&mut self, ui: &mut Ui) {
        let mut actions_out = Vec::new();
        ui.horizontal(|ui| {
            if let Some(job) = &self.delete_job {
                ui.spinner();
                let verb = match job.mode {
                    DeleteMode::RecycleBin => "Moving to Recycle Bin…",
                    DeleteMode::Permanent => "Deleting…",
                };
                ui.label(format!("{verb} {} / {}", job.progress(), job.total));
                ui.separator();
            }
            if let Some(d) = &self.data
                && !self.selection.is_empty()
            {
                let tree = &d.tree;
                let size: u64 = self
                    .selection
                    .iter()
                    .filter(|&&id| {
                        // don't double count nested selections
                        let mut p = tree.get(id).parent;
                        while p != crate::model::tree::NO_PARENT {
                            if self.selection.contains(&p) {
                                return false;
                            }
                            p = tree.get(p).parent;
                        }
                        true
                    })
                    .map(|&id| tree.get(id).size)
                    .sum();
                ui.label(
                    RichText::new(format!(
                        "☑ {} selected · {}",
                        format::count(self.selection.len() as u64),
                        format::size(size)
                    ))
                    .strong(),
                );
                if ui.button("Show").clicked() {
                    self.view = View::Selection;
                }
                if ui
                    .add_enabled(
                        self.delete_job.is_none(),
                        egui::Button::new(
                            RichText::new("🗑 Delete selected…")
                                .color(Color32::from_rgb(0xE0, 0x60, 0x60)),
                        ),
                    )
                    .clicked()
                {
                    actions_out.push(UiAction::Delete(self.selection.iter().copied().collect()));
                }
                if ui.button("Clear").clicked() {
                    self.selection.clear();
                }
                ui.separator();
            }
            if let Some((m, t)) = &self.message {
                if t.elapsed() < Duration::from_secs(10) {
                    ui.add(egui::Label::new(m.as_str()).truncate());
                    ui.ctx().request_repaint_after(Duration::from_secs(1));
                } else {
                    self.message = None;
                }
            } else if self.data.is_none() && self.scan.is_none() {
                ui.label(RichText::new("Ready.").weak());
            }
        });
        let ctx = ui.ctx().clone();
        self.handle_actions(&ctx, actions_out);
    }

    fn central(&mut self, ui: &mut Ui) {
        let mut actions_out: Vec<UiAction> = Vec::new();
        let needs_scan = !matches!(self.view, View::Overview | View::Programs);
        if needs_scan && self.data.is_none() {
            if self.scan.is_some() {
                ui.vertical_centered(|ui| {
                    ui.add_space(80.0);
                    ui.spinner();
                    ui.label("Scanning… results appear here when the scan completes.");
                });
            } else {
                ui::no_scan_placeholder(ui);
            }
            return;
        }
        match self.view {
            View::Overview => {
                let mut ctx_holder;
                let ctx = match &self.data {
                    Some(d) => {
                        ctx_holder = Ctx {
                            data: d,
                            selection: &self.selection,
                            actions: &mut actions_out,
                        };
                        Some(&mut ctx_holder)
                    }
                    None => None,
                };
                ui::overview::show(ui, &self.drives, self.data.as_ref(), ctx);
            }
            View::Programs => {
                ui::programs::show(
                    ui,
                    &mut self.programs_view,
                    self.programs.as_deref(),
                    self.data.as_ref(),
                    &mut actions_out,
                );
            }
            View::Errors => {
                if let Some(d) = &self.data {
                    ui::errors::show(ui, &mut self.errors, d, &mut actions_out);
                }
            }
            v => {
                let Some(d) = &self.data else { return };
                let mut ctx = Ctx {
                    data: d,
                    selection: &self.selection,
                    actions: &mut actions_out,
                };
                match v {
                    View::Explorer => ui::explorer::show(ui, &mut self.explorer, &mut ctx),
                    View::Treemap => ui::treemap::show(ui, &mut self.treemap, &mut ctx),
                    View::Categories => ui::categories::show(ui, &mut self.categories, &mut ctx),
                    View::Extensions => ui::extensions::show(ui, &mut self.extensions, &mut ctx),
                    View::LargestFiles => ui::files::show(ui, &mut self.largest_files, &mut ctx),
                    View::LargestFolders => {
                        ui::files::show(ui, &mut self.largest_folders, &mut ctx)
                    }
                    View::OldFiles => ui::files::show(ui, &mut self.old_files, &mut ctx),
                    View::EmptyFolders => ui::files::show(ui, &mut self.empty_folders, &mut ctx),
                    View::Duplicates => ui::duplicates::show(ui, &mut self.duplicates, &mut ctx),
                    View::Cleanup => ui::cleanup::show(ui, &mut self.cleanup, &mut ctx),
                    View::Search => ui::search::show(ui, &mut self.search, &mut ctx),
                    View::Selection => selection_view(ui, &mut ctx),
                    _ => {}
                }
            }
        }
        let ctx = ui.ctx().clone();
        self.handle_actions(&ctx, actions_out);
    }

    fn confirm_dialog(&mut self, ui: &mut Ui) {
        let Some(c) = &mut self.confirm else { return };
        let mut decision: Option<bool> = None;
        egui::Modal::new(egui::Id::unique("confirm_delete")).show(ui.ctx(), |ui| {
            ui.set_width(520.0);
            let title = if c.contents_only {
                "Empty folder contents?"
            } else {
                "Delete items?"
            };
            ui.heading(title);
            ui.add_space(4.0);
            if !c.targets.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{} item(s) · {}",
                        c.targets.len(),
                        format::size(c.total)
                    ))
                    .strong()
                    .size(15.0),
                );
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for t in c.targets.iter().take(200) {
                            let prefix = if t.contents_only {
                                "✨ contents of "
                            } else {
                                ""
                            };
                            ui.label(
                                RichText::new(format!(
                                    "{prefix}{}  ({})",
                                    t.path.display(),
                                    format::size(t.size)
                                ))
                                .small(),
                            );
                        }
                        if c.targets.len() > 200 {
                            ui.label(
                                RichText::new(format!("…and {} more", c.targets.len() - 200))
                                    .weak(),
                            );
                        }
                    });
            }
            if !c.protected.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "🛡 {} protected item(s) will be skipped:",
                        c.protected.len()
                    ))
                    .color(Color32::from_rgb(0xE0, 0xA0, 0x3C)),
                );
                for (p, reason) in c.protected.iter().take(5) {
                    ui.label(
                        RichText::new(format!("{} — {reason}", p.display()))
                            .small()
                            .weak(),
                    );
                }
            }
            ui.add_space(8.0);
            if !c.targets.is_empty() {
                ui.radio_value(
                    &mut c.mode,
                    DeleteMode::RecycleBin,
                    "♻ Move to Recycle Bin (can be restored)",
                );
                ui.radio_value(
                    &mut c.mode,
                    DeleteMode::Permanent,
                    "⚠ Delete permanently (cannot be undone)",
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if !c.targets.is_empty() {
                    let label = match c.mode {
                        DeleteMode::RecycleBin => RichText::new("Move to Recycle Bin"),
                        DeleteMode::Permanent => RichText::new("Delete permanently")
                            .color(Color32::from_rgb(0xFF, 0x70, 0x70)),
                    };
                    if ui.button(label.strong()).clicked() {
                        decision = Some(true);
                    }
                }
                if ui.button("Cancel").clicked() {
                    decision = Some(false);
                }
            });
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            decision = Some(false);
        }
        match decision {
            Some(true) => {
                let c = self.confirm.take().unwrap();
                self.delete_job = Some(DeleteJob::start(c.targets, c.mode, ui.ctx().clone()));
            }
            Some(false) => self.confirm = None,
            None => {}
        }
    }
}

fn selection_view(ui: &mut Ui, ctx: &mut Ctx) {
    ui.heading("Selection");
    ui.label(
        RichText::new(
            "Items you ticked in any view. Delete them together from the bar at the bottom.",
        )
        .weak(),
    );
    let tree = &ctx.data.tree;
    let mut rows: Vec<NodeId> = ctx
        .selection
        .iter()
        .copied()
        .filter(|&id| !tree.is_gone(id))
        .collect();
    rows.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).size));
    if rows.is_empty() {
        ui.add_space(20.0);
        ui.label("Nothing selected yet. Tick the checkbox next to files or folders in any list.");
        return;
    }
    ui.separator();
    let out = ui::node_table(
        ui,
        "selection_table",
        ctx,
        &rows,
        TableOptions {
            show_files: true,
            ..Default::default()
        },
    );
    if let Some(id) = out.activated {
        ctx.actions.push(UiAction::ShowInExplorer(id));
    }
}

fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    let norm = |p: &std::path::Path| {
        p.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn save_dialog(name: &str, ext: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(ext.to_uppercase(), &[ext])
        .save_file()
}

impl eframe::App for PcScannerApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll_background();

        egui::Panel::top("top_bar").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("bottom_bar").show(ui, |ui| self.bottom_bar(ui));
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(190.0)
            .show(ui, |ui| self.nav_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.central(ui));
        self.confirm_dialog(ui);
    }
}
