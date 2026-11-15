//! Installed, Store and portable (not installed) programs.

use std::collections::HashMap;

use egui::{Color32, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use super::{ScanData, UiAction};
use crate::model::NodeId;
use crate::system::actions;
use crate::system::programs::{Program, ProgramSource};
use crate::util::format;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Installed,
    Store,
    Portable,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    Size,
    Name,
    Publisher,
    Date,
}

pub struct PortableGroup {
    pub folder: NodeId,
    pub exes: Vec<NodeId>,
}

#[derive(Default)]
pub struct ProgramsState {
    pub tab: Tab,
    pub search: String,
    pub sort: SortBy,
    pub show_system: bool,
    confirm_uninstall: Option<Program>,
    /// Scanned size of each program's install location, keyed by scan generation.
    sizes: HashMap<usize, (NodeId, u64)>,
    sizes_key: Option<(u64, usize)>,
    portable: Vec<PortableGroup>,
    portable_key: Option<u64>,
}

impl ProgramsState {
    fn refresh_sizes(&mut self, programs: &[Program], data: Option<&ScanData>) {
        let key = (data.map(|d| d.generation + 1).unwrap_or(0), programs.len());
        if self.sizes_key == Some(key) {
            return;
        }
        self.sizes.clear();
        if let Some(d) = data {
            for (i, p) in programs.iter().enumerate() {
                if let Some(loc) = &p.install_location
                    && let Some(id) = d.tree.find_path(loc)
                    && d.tree.get(id).parent != crate::model::tree::NO_PARENT
                {
                    self.sizes.insert(i, (id, d.tree.get(id).size));
                }
            }
        }
        self.sizes_key = Some(key);
    }

    fn size_of(&self, i: usize, p: &Program) -> u64 {
        self.sizes.get(&i).map(|s| s.1).unwrap_or(p.estimated_size)
    }
}

pub fn show(ui: &mut Ui, st: &mut ProgramsState, programs: Option<&[Program]>, data: Option<&ScanData>, actions_out: &mut Vec<UiAction>) {
    ui.heading("Programs");
    let Some(programs) = programs else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Reading installed programs…");
        });
        return;
    };
    st.refresh_sizes(programs, data);

    let installed: Vec<usize> = programs
        .iter()
        .enumerate()
        .filter(|(_, p)| p.source != ProgramSource::Store && (st.show_system || !p.system_component))
        .map(|(i, _)| i)
        .collect();
    let store: Vec<usize> = programs.iter().enumerate().filter(|(_, p)| p.source == ProgramSource::Store).map(|(i, _)| i).collect();

    if let Some(d) = data
        && st.portable_key != Some(d.generation)
    {
        st.portable = find_portable(d, programs);
        st.portable_key = Some(d.generation);
    }

    ui.horizontal(|ui| {
        let inst_size: u64 = installed.iter().map(|&i| st.size_of(i, &programs[i])).sum();
        ui.selectable_value(&mut st.tab, Tab::Installed, format!("⚙ Installed ({}, {})", installed.len(), format::size(inst_size)));
        ui.selectable_value(&mut st.tab, Tab::Store, format!("⬇ Store apps ({})", store.len()));
        let portable_label = if data.is_some() { format!("📦 Not installed / portable ({})", st.portable.len()) } else { "📦 Not installed / portable".to_owned() };
        ui.selectable_value(&mut st.tab, Tab::Portable, portable_label);
        ui.separator();
        if cfg!(windows) && ui.button("⚙ Windows Apps settings").clicked()
            && let Err(e) = actions::open_apps_settings()
        {
            actions_out.push(UiAction::Notify(e));
        }
    });
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(egui::TextEdit::singleline(&mut st.search).hint_text("Name or publisher").desired_width(220.0));
        if st.tab != Tab::Portable {
            ui.label("Sort:");
            ui.selectable_value(&mut st.sort, SortBy::Size, "Size");
            ui.selectable_value(&mut st.sort, SortBy::Name, "Name");
            ui.selectable_value(&mut st.sort, SortBy::Publisher, "Publisher");
            ui.selectable_value(&mut st.sort, SortBy::Date, "Install date");
        }
        if st.tab == Tab::Installed {
            ui.checkbox(&mut st.show_system, "Show system components");
        }
    });
    if data.is_none() && st.tab != Tab::Store {
        ui.label(RichText::new("Sizes are as reported by installers. Run a scan to measure real folder sizes and find portable programs.").weak());
    }
    ui.separator();

    let search = st.search.to_lowercase();
    match st.tab {
        Tab::Installed | Tab::Store => {
            let mut rows: Vec<usize> = if st.tab == Tab::Installed { installed } else { store };
            if !search.is_empty() {
                rows.retain(|&i| {
                    programs[i].name.to_lowercase().contains(&search) || programs[i].publisher.to_lowercase().contains(&search)
                });
            }
            match st.sort {
                SortBy::Size => rows.sort_by_key(|&i| std::cmp::Reverse(st.size_of(i, &programs[i]))),
                SortBy::Name => rows.sort_by_cached_key(|&i| programs[i].name.to_lowercase()),
                SortBy::Publisher => rows.sort_by_cached_key(|&i| programs[i].publisher.to_lowercase()),
                SortBy::Date => rows.sort_by(|&a, &b| programs[b].install_date.cmp(&programs[a].install_date)),
            }
            program_table(ui, st, programs, &rows, data, actions_out);
        }
        Tab::Portable => {
            let Some(d) = data else {
                ui.label("Run a scan first — portable programs are found by looking for executables outside of installed locations.");
                return;
            };
            portable_table(ui, st, d, &search, actions_out);
        }
    }

    // Uninstall confirmation
    if let Some(p) = st.confirm_uninstall.clone() {
        let mut close = false;
        egui::Modal::new(egui::Id::unique("confirm_uninstall")).show(ui.ctx(), |ui| {
            ui.set_width(420.0);
            ui.heading("Uninstall program?");
            ui.label(RichText::new(&p.name).strong());
            if !p.publisher.is_empty() {
                ui.label(format!("by {}", p.publisher));
            }
            ui.add_space(6.0);
            ui.label("This starts the program's own uninstaller. Follow its instructions to finish.");
            if let Some(u) = &p.uninstall_string {
                ui.label(RichText::new(u).monospace().small().weak());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Run uninstaller").color(Color32::from_rgb(0xE0, 0x60, 0x60))).clicked() {
                    if let Some(u) = &p.uninstall_string {
                        match actions::run_uninstaller(u) {
                            Ok(()) => actions_out.push(UiAction::Notify(format!("Started uninstaller for {}. Rescan afterwards to refresh sizes.", p.name))),
                            Err(e) => actions_out.push(UiAction::Notify(format!("Could not start uninstaller: {e}"))),
                        }
                    }
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
        if close {
            st.confirm_uninstall = None;
        }
    }
}

fn program_table(ui: &mut Ui, st: &mut ProgramsState, programs: &[Program], rows: &[usize], data: Option<&ScanData>, actions_out: &mut Vec<UiAction>) {
    let total: u64 = rows.iter().map(|&i| st.size_of(i, &programs[i])).sum();
    ui.label(RichText::new(format!("{} programs · {}", rows.len(), format::size(total))).strong());
    let mut uninstall: Option<Program> = None;
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(300.0).clip(true))
        .column(Column::initial(110.0).clip(true))
        .column(Column::initial(180.0).clip(true))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::initial(140.0))
        .column(Column::remainder().clip(true))
        .header(20.0, |mut h| {
            for t in ["Name", "Version", "Publisher", "Size", "Installed", "Type", "Location"] {
                h.col(|ui| {
                    ui.strong(t);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows.len(), |mut row| {
                let i = rows[row.index()];
                let p = &programs[i];
                let measured = st.sizes.get(&i).copied();
                row.col(|ui| {
                    ui.add(egui::Label::new(RichText::new(&p.name).strong()).truncate().selectable(false));
                });
                row.col(|ui| {
                    ui.label(&p.version);
                });
                row.col(|ui| {
                    ui.add(egui::Label::new(&p.publisher).truncate().selectable(false));
                });
                row.col(|ui| {
                    let size = measured.map(|m| m.1).unwrap_or(p.estimated_size);
                    if size == 0 {
                        ui.label(RichText::new("—").weak());
                    } else if measured.is_some() {
                        ui.label(format::size(size)).on_hover_text("Measured from the scan");
                    } else {
                        ui.label(RichText::new(format::size(size)).italics()).on_hover_text("Estimated by the installer");
                    }
                });
                row.col(|ui| {
                    ui.label(p.install_date_display());
                });
                row.col(|ui| {
                    ui.label(RichText::new(p.source.label()).weak());
                });
                row.col(|ui| {
                    if let Some(l) = &p.install_location {
                        ui.add(egui::Label::new(RichText::new(l.to_string_lossy()).weak()).truncate().selectable(false));
                    }
                });
                let resp = row.response();
                if resp.double_clicked()
                    && let Some(l) = &p.install_location
                {
                    actions_out.push(UiAction::Open(l.clone()));
                }
                resp.context_menu(|ui| {
                    ui.label(RichText::new(&p.name).strong());
                    ui.separator();
                    if let Some(l) = &p.install_location {
                        if ui.button("📂 Open install folder").clicked() {
                            actions_out.push(UiAction::Open(l.clone()));
                            ui.close();
                        }
                        if ui.button("🔎 Show in file manager").clicked() {
                            actions_out.push(UiAction::Reveal(l.clone()));
                            ui.close();
                        }
                        if let Some((id, _)) = measured
                            && data.is_some()
                            && ui.button("🔭 Browse in PC Scanner").clicked()
                        {
                            actions_out.push(UiAction::ShowInExplorer(id));
                            ui.close();
                        }
                        if ui.button("📋 Copy location").clicked() {
                            actions_out.push(UiAction::CopyText(l.to_string_lossy().into_owned()));
                            ui.close();
                        }
                    } else {
                        ui.label(RichText::new("Install location unknown").weak());
                    }
                    if ui.button("📋 Copy name").clicked() {
                        actions_out.push(UiAction::CopyText(format!("{} {}", p.name, p.version)));
                        ui.close();
                    }
                    ui.separator();
                    if p.uninstall_string.is_some() {
                        if ui.button(RichText::new("🗑 Uninstall…").color(Color32::from_rgb(0xE0, 0x60, 0x60))).clicked() {
                            uninstall = Some(p.clone());
                            ui.close();
                        }
                    } else if p.source == ProgramSource::Store && ui.button("⚙ Manage in Windows settings").clicked() {
                        if let Err(e) = actions::open_apps_settings() {
                            actions_out.push(UiAction::Notify(e));
                        }
                        ui.close();
                    }
                });
            });
        });
    if uninstall.is_some() {
        st.confirm_uninstall = uninstall;
    }
}

fn portable_table(ui: &mut Ui, st: &mut ProgramsState, data: &ScanData, search: &str, actions_out: &mut Vec<UiAction>) {
    let tree = &data.tree;
    ui.label(RichText::new(
        "Executables found outside Windows, Program Files and registered install folders — portable apps, \
         games, extracted tools and leftover setup files.",
    ).weak());
    let rows: Vec<&PortableGroup> = st
        .portable
        .iter()
        .filter(|g| !tree.is_gone(g.folder))
        .filter(|g| {
            search.is_empty()
                || tree.get(g.folder).name.to_lowercase().contains(search)
                || g.exes.iter().any(|&e| tree.get(e).name.to_lowercase().contains(search))
        })
        .collect();
    ui.label(RichText::new(format!("{} folders", rows.len())).strong());
    let mut sel_actions: Vec<UiAction> = Vec::new();
    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .column(Column::initial(220.0).clip(true))
        .column(Column::initial(280.0).clip(true))
        .column(Column::initial(90.0))
        .column(Column::initial(100.0))
        .column(Column::remainder().clip(true))
        .header(20.0, |mut h| {
            for t in ["Folder", "Executables", "Folder size", "Modified", "Location"] {
                h.col(|ui| {
                    ui.strong(t);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows.len(), |mut row| {
                let g = rows[row.index()];
                let f = tree.get(g.folder);
                row.col(|ui| {
                    ui.add(egui::Label::new(RichText::new(format!("📁 {}", f.name)).strong()).truncate().selectable(false));
                });
                row.col(|ui| {
                    let names: Vec<&str> = g.exes.iter().take(4).map(|&e| &*tree.get(e).name).collect();
                    let more = if g.exes.len() > 4 { format!(" +{}", g.exes.len() - 4) } else { String::new() };
                    ui.add(egui::Label::new(format!("{}{}", names.join(", "), more)).truncate().selectable(false));
                });
                row.col(|ui| {
                    ui.label(format::size(f.size));
                });
                row.col(|ui| {
                    ui.label(format::date_short(f.modified));
                });
                row.col(|ui| {
                    ui.add(egui::Label::new(RichText::new(tree.path(g.folder).to_string_lossy()).weak()).truncate().selectable(false));
                });
                let resp = row.response();
                if resp.double_clicked() {
                    sel_actions.push(UiAction::ShowInExplorer(g.folder));
                }
                resp.context_menu(|ui| {
                    for &e in g.exes.iter().take(10) {
                        if ui.button(format!("▶ Run {}", tree.get(e).name)).clicked() {
                            sel_actions.push(UiAction::Open(tree.path(e)));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("📂 Open folder").clicked() {
                        sel_actions.push(UiAction::Open(tree.path(g.folder)));
                        ui.close();
                    }
                    if ui.button("🔎 Show in file manager").clicked() {
                        sel_actions.push(UiAction::Reveal(tree.path(g.exes[0])));
                        ui.close();
                    }
                    if ui.button("🔭 Browse in PC Scanner").clicked() {
                        sel_actions.push(UiAction::ShowInExplorer(g.folder));
                        ui.close();
                    }
                    if ui.button("📋 Copy path").clicked() {
                        sel_actions.push(UiAction::CopyText(tree.path(g.folder).to_string_lossy().into_owned()));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(RichText::new("🗑 Delete folder…").color(Color32::from_rgb(0xE0, 0x60, 0x60))).clicked() {
                        sel_actions.push(UiAction::Delete(vec![g.folder]));
                        ui.close();
                    }
                });
            });
        });
    actions_out.extend(sel_actions);
}

/// Executables that do not belong to Windows or a registered installation, grouped by folder.
fn find_portable(data: &ScanData, programs: &[Program]) -> Vec<PortableGroup> {
    let tree = &data.tree;
    let norm = |s: String| s.to_lowercase().replace('/', "\\");
    let mut excluded: Vec<String> = programs
        .iter()
        .filter_map(|p| p.install_location.as_ref())
        .map(|l| norm(l.to_string_lossy().into_owned()))
        .filter(|l| l.len() > 3)
        .map(|l| if l.ends_with('\\') { l } else { l + "\\" })
        .collect();
    #[cfg(windows)]
    {
        let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        excluded.push(norm(sysroot) + "\\");
        for v in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] {
            if let Ok(p) = std::env::var(v) {
                excluded.push(norm(p) + "\\");
            }
        }
    }
    const NOISE: [&str; 14] = [
        "\\$recycle.bin\\", "\\node_modules\\", "\\.cargo\\", "\\.rustup\\", "\\target\\", "\\temp\\",
        "\\appdata\\local\\microsoft\\", "\\appdata\\local\\packages\\", "\\windowsapps\\", "\\.vscode\\extensions\\",
        "\\system volume information\\", "\\appdata\\local\\temp\\", "\\.git\\", "\\site-packages\\",
    ];
    let mut groups: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for &id in &data.analysis.executables {
        if tree.is_gone(id) {
            continue;
        }
        let p = norm(tree.path(id).to_string_lossy().into_owned());
        if excluded.iter().any(|e| p.starts_with(e.as_str())) || NOISE.iter().any(|n| p.contains(n)) {
            continue;
        }
        let parent = tree.get(id).parent;
        groups.entry(parent).or_default().push(id);
    }
    let mut out: Vec<PortableGroup> = groups
        .into_iter()
        .map(|(folder, mut exes)| {
            exes.sort_by_key(|&e| std::cmp::Reverse(tree.get(e).size));
            PortableGroup { folder, exes }
        })
        .collect();
    out.sort_by_key(|g| std::cmp::Reverse(tree.get(g.folder).size));
    out
}
