//! Squarified treemap of a folder.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

use super::{Ctx, UiAction};
use crate::model::{FileTree, NodeId};
use crate::util::format;

pub struct TreemapState {
    pub current: Option<NodeId>,
    pub depth: usize,
    menu_node: Option<NodeId>,
}

impl Default for TreemapState {
    fn default() -> Self {
        Self { current: None, depth: 3, menu_node: None }
    }
}

const MAX_RECTS: usize = 6000;
const HEADER: f32 = 15.0;

struct Cell {
    id: NodeId,
    rect: Rect,
    depth: usize,
}

pub fn show(ui: &mut Ui, state: &mut TreemapState, ctx: &mut Ctx) {
    let data = ctx.data;
    let tree = &data.tree;
    if state.current.is_some_and(|c| tree.is_gone(c)) {
        state.current = None;
    }
    let current = match state.current {
        Some(c) => c,
        None => match tree.roots.iter().copied().find(|&r| !tree.get(r).deleted) {
            Some(r) if tree.roots.len() == 1 => r,
            Some(_) => {
                // multiple roots: let user pick
                ui.heading("Treemap");
                ui.label("Pick a scanned location:");
                for &r in &tree.roots {
                    let n = tree.get(r);
                    if ui.button(format!("🖴 {}  —  {}", n.name, format::size(n.size))).clicked() {
                        state.current = Some(r);
                    }
                }
                return;
            }
            None => return,
        },
    };

    ui.horizontal(|ui| {
        let parent = tree.get(current).parent;
        if ui.add_enabled(parent != crate::model::tree::NO_PARENT, egui::Button::new("⬆ Up")).clicked() {
            state.current = Some(parent);
        }
        if tree.roots.len() > 1 && ui.button("🏠").on_hover_text("All locations").clicked() {
            state.current = None;
        }
        ui.separator();
        for id in tree.ancestry(current) {
            if id != tree.root_of(current) {
                ui.label("›");
            }
            if ui.link(tree.get(id).name.to_string()).clicked() {
                state.current = Some(id);
            }
        }
        ui.separator();
        ui.label("Depth");
        ui.add(egui::Slider::new(&mut state.depth, 1..=6));
        ui.label(RichText::new(format::size(tree.get(current).size)).strong());
    });
    ui.label(RichText::new("Click a folder to zoom in · double-click a file to open · right-click for actions").weak().small());

    let size = ui.available_size();
    let (resp, painter) = ui.allocate_painter(size, Sense::click());
    let area = resp.rect.shrink(2.0);

    let mut cells: Vec<Cell> = Vec::new();
    layout_children(tree, current, area, 0, state.depth, &mut cells);

    let hover_pos = resp.hover_pos();
    let mut hovered: Option<usize> = None;
    let dark = ui.visuals().dark_mode;
    let text_color = if dark { Color32::from_gray(235) } else { Color32::from_gray(20) };
    for (i, c) in cells.iter().enumerate() {
        let n = tree.get(c.id);
        let fill = if n.is_dir { dir_color(c.depth, dark) } else { shade(n.category.color(), c.depth) };
        painter.rect_filled(c.rect, 2.0, fill);
        painter.rect_stroke(c.rect, 2.0, Stroke::new(1.0, Color32::from_black_alpha(90)), StrokeKind::Inside);
        let has_header = n.is_dir && c.rect.height() > HEADER * 2.0 && c.rect.width() > 40.0;
        if (has_header || (!n.is_dir && c.rect.width() > 50.0 && c.rect.height() > 16.0)) && c.rect.width() > 30.0 {
            let label = format!("{}  {}", n.name, format::size(n.size));
            let clip = painter.with_clip_rect(c.rect.shrink(2.0));
            clip.text(
                c.rect.left_top() + Vec2::new(4.0, 1.0),
                Align2::LEFT_TOP,
                label,
                FontId::proportional(11.0),
                if n.is_dir { text_color } else { Color32::from_gray(15) },
            );
        }
        if let Some(p) = hover_pos
            && c.rect.contains(p)
        {
            hovered = Some(i);
        }
    }

    if let Some(i) = hovered {
        let c = &cells[i];
        painter.rect_stroke(c.rect, 2.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
        let n = tree.get(c.id);
        let path = tree.path(c.id);
        let parent_size = tree.get(current).size.max(1);
        let info = format!(
            "{}\n{} ({:.1}% of this view){}\nModified {}",
            path.display(),
            format::size(n.size),
            format::percent(n.size, parent_size),
            if n.is_dir { format!("\n{} files", format::count(n.files as u64)) } else { String::new() },
            format::age(n.modified)
        );
        let resp2 = resp.clone().on_hover_text_at_pointer(info);
        if resp2.clicked() {
            // zoom into the outermost folder below `current` that contains the pointer
            let target = outer_dir_at(&cells, tree, i);
            if let Some(t) = target {
                state.current = Some(t);
            }
        }
        if resp2.double_clicked() && !n.is_dir {
            ctx.actions.push(UiAction::Open(path));
        }
        if resp2.secondary_clicked() {
            state.menu_node = Some(c.id);
        }
    }
    if let Some(id) = state.menu_node {
        resp.context_menu(|ui| super::node_context_menu(ui, ctx, id));
    }
    if cells.is_empty() {
        painter.text(area.center(), Align2::CENTER_CENTER, "Empty folder", FontId::proportional(16.0), text_color);
    }
}

/// The depth-0 ancestor (child of `current`) for the hovered cell, if it is a folder.
fn outer_dir_at(cells: &[Cell], tree: &FileTree, hovered: usize) -> Option<NodeId> {
    let mut id = cells[hovered].id;
    let depth = cells[hovered].depth;
    for _ in 0..depth {
        id = tree.get(id).parent;
    }
    if tree.get(id).is_dir { Some(id) } else { None }
}

fn layout_children(tree: &FileTree, dir: NodeId, rect: Rect, depth: usize, max_depth: usize, out: &mut Vec<Cell>) {
    if out.len() >= MAX_RECTS || rect.width() < 2.0 || rect.height() < 2.0 {
        return;
    }
    let total = tree.get(dir).size;
    if total == 0 {
        return;
    }
    // Children are sorted by size already; ignore those that would be smaller than ~3 px².
    let min_size = (total as f64 * 3.0 / (rect.area() as f64).max(1.0)) as u64;
    let items: Vec<(NodeId, f64)> = tree
        .children(dir)
        .filter(|&c| tree.get(c).size > min_size)
        .take(800)
        .map(|c| (c, tree.get(c).size as f64))
        .collect();
    if items.is_empty() {
        return;
    }
    let mut placed = Vec::with_capacity(items.len());
    squarify(&items, rect, total as f64, &mut placed);
    for (id, r) in placed {
        if out.len() >= MAX_RECTS {
            return;
        }
        out.push(Cell { id, rect: r, depth });
        let n = tree.get(id);
        if n.is_dir && depth + 1 < max_depth && r.width() > 30.0 && r.height() > HEADER + 16.0 {
            let inner = Rect::from_min_max(pos2(r.left() + 2.0, r.top() + HEADER), pos2(r.right() - 2.0, r.bottom() - 2.0));
            layout_children(tree, id, inner, depth + 1, max_depth, out);
        }
    }
}

/// Squarified treemap layout (Bruls et al.). `total` is the value that maps to the whole rect.
pub fn squarify(items: &[(NodeId, f64)], rect: Rect, total: f64, out: &mut Vec<(NodeId, Rect)>) {
    if total <= 0.0 || items.is_empty() {
        return;
    }
    let scale = (rect.width() as f64 * rect.height() as f64) / total;
    let areas: Vec<f64> = items.iter().map(|(_, s)| s * scale).collect();
    let mut x0 = rect.left() as f64;
    let mut y0 = rect.top() as f64;
    let x1 = rect.right() as f64;
    let y1 = rect.bottom() as f64;
    let mut i = 0;
    while i < areas.len() {
        let w = x1 - x0;
        let h = y1 - y0;
        if w <= 0.5 || h <= 0.5 {
            break;
        }
        let short = w.min(h);
        let mut end = i + 1;
        let mut sum = areas[i];
        let mut best = worst(areas[i], areas[i], sum, short);
        while end < areas.len() {
            let ns = sum + areas[end];
            let max = areas[i]; // sorted desc
            let min = areas[end];
            let wr = worst(max, min, ns, short);
            if wr > best {
                break;
            }
            best = wr;
            sum = ns;
            end += 1;
        }
        if w >= h {
            // column on the left
            let col_w = (sum / h).min(w);
            let mut y = y0;
            for k in i..end {
                let hh = areas[k] / col_w;
                out.push((items[k].0, Rect::from_min_max(pos2(x0 as f32, y as f32), pos2((x0 + col_w) as f32, (y + hh) as f32))));
                y += hh;
            }
            x0 += col_w;
        } else {
            let row_h = (sum / w).min(h);
            let mut x = x0;
            for k in i..end {
                let ww = areas[k] / row_h;
                out.push((items[k].0, Rect::from_min_max(pos2(x as f32, y0 as f32), pos2((x + ww) as f32, (y0 + row_h) as f32))));
                x += ww;
            }
            y0 += row_h;
        }
        i = end;
    }
}

fn worst(max: f64, min: f64, sum: f64, side: f64) -> f64 {
    let s2 = sum * sum;
    let w2 = side * side;
    (w2 * max / s2).max(s2 / (w2 * min))
}

fn dir_color(depth: usize, dark: bool) -> Color32 {
    let base: u8 = if dark { 55 } else { 200 };
    let step = (depth as u8).saturating_mul(12);
    let v = if dark { base.saturating_add(step) } else { base.saturating_sub(step) };
    Color32::from_rgb(v.saturating_sub(5), v, v.saturating_add(14))
}

fn shade(c: Color32, depth: usize) -> Color32 {
    let f = 1.0 - (depth as f32 * 0.06).min(0.3);
    Color32::from_rgb((c.r() as f32 * f) as u8, (c.g() as f32 * f) as u8, (c.b() as f32 * f) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_fills_rect() {
        let items: Vec<(NodeId, f64)> = vec![(0, 6.0), (1, 6.0), (2, 4.0), (3, 3.0), (4, 2.0), (5, 2.0), (6, 1.0)];
        let rect = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(600.0, 400.0));
        let mut out = Vec::new();
        squarify(&items, rect, 24.0, &mut out);
        assert_eq!(out.len(), 7);
        let area: f32 = out.iter().map(|(_, r)| r.area()).sum();
        assert!((area - rect.area()).abs() < 1.0, "area {area}");
        for (_, r) in &out {
            assert!(rect.expand(0.01).contains_rect(*r));
        }
    }
}
