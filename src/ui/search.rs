//! Search all scanned files and folders by name.

use egui::{RichText, Ui};

use super::{Ctx, TableOptions, UiAction, node_table};
use crate::model::NodeId;
use crate::util::format;

pub struct SearchState {
    pub query: String,
    pub include_dirs: bool,
    pub include_files: bool,
    pub results: Vec<NodeId>,
    pub searched: Option<(String, u64)>,
    pub truncated: bool,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            include_dirs: true,
            include_files: true,
            results: Vec::new(),
            searched: None,
            truncated: false,
        }
    }
}

const MAX_RESULTS: usize = 50_000;

pub fn show(ui: &mut Ui, st: &mut SearchState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    ui.heading("Search");
    let mut run = false;
    ui.horizontal(|ui| {
        let resp = ui.add(
            egui::TextEdit::singleline(&mut st.query)
                .hint_text("File or folder name (use * as wildcard, e.g. *.psd or invoice*2024)")
                .desired_width(420.0),
        );
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            run = true;
        }
        if ui.button("🔍 Search").clicked() {
            run = true;
        }
        ui.checkbox(&mut st.include_files, "Files");
        ui.checkbox(&mut st.include_dirs, "Folders");
    });
    if run && !st.query.trim().is_empty() {
        let pattern = Pattern::new(&st.query);
        let mut results: Vec<NodeId> = Vec::new();
        st.truncated = false;
        for (i, n) in tree.nodes.iter().enumerate() {
            if n.deleted || (n.is_dir && !st.include_dirs) || (!n.is_dir && !st.include_files) {
                continue;
            }
            if pattern.matches(&n.name) {
                results.push(i as NodeId);
                if results.len() >= MAX_RESULTS {
                    st.truncated = true;
                    break;
                }
            }
        }
        results.sort_unstable_by_key(|&id| std::cmp::Reverse(tree.get(id).size));
        st.results = results;
        st.searched = Some((st.query.clone(), data.generation));
    }
    if st.searched.is_some() {
        st.results.retain(|&id| !tree.is_gone(id));
        let total: u64 = st.results.iter().map(|&id| tree.get(id).size).sum();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} results · {}",
                    format::count(st.results.len() as u64),
                    format::size(total)
                ))
                .strong(),
            );
            if st.truncated {
                ui.label(
                    RichText::new(format!(
                        "(stopped at {} results)",
                        format::count(MAX_RESULTS as u64)
                    ))
                    .weak(),
                );
            }
            if !st.results.is_empty() && ui.button("☑ Select all results").clicked() {
                ctx.actions.push(UiAction::Select(st.results.clone()));
            }
        });
        ui.separator();
        let rows = std::mem::take(&mut st.results);
        let out = node_table(
            ui,
            "search_table",
            ctx,
            &rows,
            TableOptions {
                show_files: true,
                ..Default::default()
            },
        );
        if let Some(id) = out.activated {
            if tree.get(id).is_dir {
                ctx.actions.push(UiAction::ShowInExplorer(id));
            } else {
                ctx.actions.push(UiAction::Open(tree.path(id)));
            }
        }
        st.results = rows;
    }
}

/// Simple case-insensitive matcher: substring, or glob with `*` and `?`.
pub struct Pattern {
    glob: bool,
    raw: String,
}

impl Pattern {
    pub fn new(q: &str) -> Self {
        let raw = q.trim().to_lowercase();
        let glob = raw.contains('*') || raw.contains('?');
        Self { glob, raw }
    }

    pub fn matches(&self, name: &str) -> bool {
        let name = name.to_lowercase();
        if !self.glob {
            return name.contains(&self.raw);
        }
        glob_match(&self.raw, &name)
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        assert!(Pattern::new("report").matches("Annual_REPORT.pdf"));
        assert!(Pattern::new("*.psd").matches("design.PSD"));
        assert!(!Pattern::new("*.psd").matches("design.psd.bak"));
        assert!(Pattern::new("inv*2024*").matches("invoice-march-2024.pdf"));
        assert!(Pattern::new("a?c").matches("abc"));
        assert!(!Pattern::new("a?c").matches("abbc"));
    }
}
