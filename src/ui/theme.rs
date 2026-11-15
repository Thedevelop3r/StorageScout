//! Look and feel: colours, fonts, egui visuals and the custom widgets of the v2 design
//! (cards, ring charts, gauges, sidebar items, buttons).

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use egui::epaint::{CornerRadius, Shadow, Stroke};
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2, Rect, Response, RichText,
    Sense, TextStyle, Ui, Vec2, pos2, vec2,
};

/// Accent colours offered in Settings (all light enough for dark text on top).
#[allow(dead_code)] // used by the Settings page
pub const ACCENTS: [(&str, Color32); 4] = [
    ("Cyan", Color32::from_rgb(0x5E, 0xE6, 0xFF)),
    ("Violet", Color32::from_rgb(0x8B, 0x7C, 0xFF)),
    ("Mint", Color32::from_rgb(0x6E, 0xE7, 0xB7)),
    ("Amber", Color32::from_rgb(0xFF, 0xB4, 0x54)),
];

static ACCENT: AtomicU32 = AtomicU32::new(0x5EE6FF);

#[allow(dead_code)]
pub fn set_accent(c: Color32) {
    ACCENT.store(
        ((c.r() as u32) << 16) | ((c.g() as u32) << 8) | c.b() as u32,
        Ordering::Relaxed,
    );
}

pub fn accent() -> Color32 {
    let v = ACCENT.load(Ordering::Relaxed);
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Colours of the current theme.
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub sidebar: Color32,
    pub card: Color32,
    pub card_hi: Color32,
    pub border: Color32,
    pub border_hi: Color32,
    pub track: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    /// Accent for text and thin lines (darker in light mode for contrast).
    pub accent_text: Color32,
    /// Subtle accent-tinted surface (selected sidebar item, chips).
    pub accent_bg: Color32,
    pub on_accent: Color32,
    pub violet: Color32,
    pub success: Color32,
    #[allow(dead_code)]
    pub success_bg: Color32,
    pub warning: Color32,
    pub warning_bg: Color32,
    pub danger: Color32,
    pub danger_bg: Color32,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

pub fn palette(dark: bool) -> Palette {
    let accent = accent();
    if dark {
        let card = Color32::from_rgb(0x0E, 0x13, 0x1C);
        Palette {
            dark,
            bg: Color32::from_rgb(0x08, 0x0B, 0x11),
            sidebar: Color32::from_rgb(0x0C, 0x10, 0x18),
            card,
            card_hi: Color32::from_rgb(0x11, 0x18, 0x24),
            border: Color32::from_rgb(0x1B, 0x23, 0x32),
            border_hi: Color32::from_rgb(0x2A, 0x34, 0x46),
            track: Color32::from_rgb(0x1A, 0x21, 0x30),
            text: Color32::from_rgb(0xE6, 0xEB, 0xF4),
            text_dim: Color32::from_rgb(0xA9, 0xB3, 0xC5),
            text_muted: Color32::from_rgb(0x87, 0x92, 0xA6),
            accent,
            accent_text: accent,
            accent_bg: mix(card, accent, 0.12),
            on_accent: Color32::from_rgb(0x04, 0x1A, 0x22),
            violet: Color32::from_rgb(0x8B, 0x7C, 0xFF),
            success: Color32::from_rgb(0x6E, 0xE7, 0xB7),
            success_bg: Color32::from_rgb(0x0F, 0x2A, 0x22),
            warning: Color32::from_rgb(0xFD, 0xBA, 0x74),
            warning_bg: Color32::from_rgb(0x2A, 0x1A, 0x12),
            danger: Color32::from_rgb(0xFC, 0xA5, 0xA5),
            danger_bg: Color32::from_rgb(0x2A, 0x14, 0x16),
        }
    } else {
        let card = Color32::WHITE;
        Palette {
            dark,
            bg: Color32::from_rgb(0xF3, 0xF5, 0xF9),
            sidebar: Color32::from_rgb(0xFB, 0xFC, 0xFE),
            card,
            card_hi: Color32::from_rgb(0xF1, 0xF4, 0xF9),
            border: Color32::from_rgb(0xE2, 0xE7, 0xEF),
            border_hi: Color32::from_rgb(0xC9, 0xD2, 0xDF),
            track: Color32::from_rgb(0xE6, 0xEA, 0xF1),
            text: Color32::from_rgb(0x11, 0x18, 0x27),
            text_dim: Color32::from_rgb(0x47, 0x51, 0x63),
            text_muted: Color32::from_rgb(0x5F, 0x6B, 0x7D),
            accent,
            accent_text: mix(accent, Color32::BLACK, 0.5),
            accent_bg: mix(card, accent, 0.16),
            on_accent: Color32::from_rgb(0x04, 0x1A, 0x22),
            violet: Color32::from_rgb(0x5B, 0x4B, 0xD6),
            success: Color32::from_rgb(0x04, 0x78, 0x57),
            success_bg: Color32::from_rgb(0xD8, 0xF6, 0xEA),
            warning: Color32::from_rgb(0xA1, 0x4A, 0x08),
            warning_bg: Color32::from_rgb(0xFE, 0xF0, 0xDC),
            danger: Color32::from_rgb(0xB9, 0x1C, 0x1C),
            danger_bg: Color32::from_rgb(0xFE, 0xE4, 0xE4),
        }
    }
}

/// Palette for the theme `ui` is drawn with.
pub fn pal(ui: &Ui) -> Palette {
    palette(ui.visuals().dark_mode)
}

// ----------------------------------------------------------------------------- fonts

pub fn heading_family() -> FontFamily {
    FontFamily::Name("heading".into())
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Space Grotesk, for titles and big numbers.
pub fn heading(size: f32) -> FontId {
    FontId::new(size, heading_family())
}

/// Inter SemiBold.
pub fn bold(size: f32) -> FontId {
    FontId::new(size, bold_family())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let icon_tweak = egui::FontTweak {
        scale: 1.05,
        y_offset_factor: 0.12,
        ..Default::default()
    };
    let add = |fonts: &mut FontDefinitions, name: &str, data: FontData| {
        fonts.font_data.insert(name.to_owned(), Arc::new(data));
    };
    add(
        &mut fonts,
        "inter",
        FontData::from_static(include_bytes!("../../assets/fonts/Inter-Regular.ttf")),
    );
    add(
        &mut fonts,
        "inter-semibold",
        FontData::from_static(include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")),
    );
    add(
        &mut fonts,
        "grotesk",
        FontData::from_static(include_bytes!(
            "../../assets/fonts/SpaceGrotesk-SemiBold.ttf"
        )),
    );
    add(
        &mut fonts,
        "phosphor",
        FontData::from_static(include_bytes!("../../assets/fonts/Phosphor.ttf")).tweak(icon_tweak),
    );

    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let with = |first: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = first.iter().map(|s| s.to_string()).collect();
        v.push("phosphor".into());
        v.extend(defaults.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, with(&["inter"]));
    fonts
        .families
        .insert(bold_family(), with(&["inter-semibold"]));
    fonts
        .families
        .insert(heading_family(), with(&["grotesk", "inter-semibold"]));
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.push("phosphor".into());
    }
    ctx.set_fonts(fonts);
}

// ----------------------------------------------------------------------------- visuals

fn visuals(p: &Palette) -> egui::Visuals {
    let mut v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let r10 = CornerRadius::same(10);
    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.window_stroke = Stroke::new(1.0, p.border_hi);
    v.window_corner_radius = CornerRadius::same(18);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 40,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 140 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 110 } else { 30 }),
    };
    v.extreme_bg_color = if p.dark {
        Color32::from_rgb(0x0B, 0x0F, 0x16)
    } else {
        Color32::from_rgb(0xF6, 0xF8, 0xFB)
    };
    v.faint_bg_color = p.card_hi;
    v.code_bg_color = p.card_hi;
    v.hyperlink_color = p.accent_text;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    v.selection.bg_fill = mix(p.card, p.accent, if p.dark { 0.28 } else { 0.35 });
    v.selection.stroke = Stroke::new(1.0, p.accent_text);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.card;
    w.noninteractive.weak_bg_fill = p.card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r10;

    w.inactive.bg_fill = p.card_hi;
    w.inactive.weak_bg_fill = p.card_hi;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r10;

    let hover = mix(p.card_hi, p.text, 0.06);
    w.hovered.bg_fill = hover;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.border_hi);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = r10;
    w.hovered.expansion = 0.0;

    let active = mix(p.card_hi, p.accent, 0.18);
    w.active.bg_fill = active;
    w.active.weak_bg_fill = active;
    w.active.bg_stroke = Stroke::new(1.0, p.accent_text);
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.active.corner_radius = r10;
    w.active.expansion = 0.0;

    w.open = w.hovered;
    v
}

/// Apply fonts sizes, spacing and both light and dark visuals.
pub fn apply(ctx: &egui::Context) {
    for dark in [true, false] {
        let theme = if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_visuals_of(theme, visuals(&palette(dark)));
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = vec2(8.0, 6.0);
            s.spacing.button_padding = vec2(12.0, 6.0);
            s.spacing.interact_size.y = 30.0;
            s.spacing.menu_margin = Margin::same(8);
            s.spacing.window_margin = Margin::same(20);
            s.text_styles = [
                (TextStyle::Small, FontId::proportional(12.0)),
                (TextStyle::Body, FontId::proportional(14.0)),
                (TextStyle::Button, FontId::proportional(14.0)),
                (TextStyle::Heading, heading(22.0)),
                (TextStyle::Monospace, FontId::monospace(13.0)),
            ]
            .into();
        });
    }
}

// ----------------------------------------------------------------------------- widgets

/// Rounded card with a hairline border.
pub fn card(ui: &Ui) -> egui::Frame {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(20))
}

/// Smaller inner card (list rows inside a card).
pub fn tile(ui: &Ui) -> egui::Frame {
    let p = pal(ui);
    egui::Frame::new()
        .fill(p.card_hi)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(14, 10))
}

/// Section title in Space Grotesk.
pub fn title(ui: &mut Ui, text: &str, size: f32) -> Response {
    let p = pal(ui);
    ui.label(RichText::new(text).font(heading(size)).color(p.text))
}

/// Points along an arc. Angles in fractions of a turn, 0 = 12 o'clock, clockwise.
pub fn arc_points(center: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    let span = (to - from).abs();
    let steps = ((span * 160.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let t = from + (to - from) * i as f32 / steps as f32;
            let a = t * std::f32::consts::TAU;
            pos2(center.x + radius * a.sin(), center.y - radius * a.cos())
        })
        .collect()
}

/// Thick arc stroke with round-ish ends drawn as caps.
#[allow(clippy::too_many_arguments)]
pub fn arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    width: f32,
    from: f32,
    to: f32,
    color: Color32,
    round_caps: bool,
) {
    if to <= from {
        return;
    }
    let pts = arc_points(center, radius, from, to);
    if round_caps {
        painter.circle_filled(pts[0], width / 2.0, color);
        painter.circle_filled(*pts.last().unwrap(), width / 2.0, color);
    }
    painter.add(egui::Shape::line(pts, Stroke::new(width, color)));
}

/// Donut chart. `segments` are (fraction, colour); returns the hovered segment index.
pub fn donut(
    ui: &mut Ui,
    size: f32,
    width: f32,
    segments: &[(f32, Color32)],
) -> (Response, Option<usize>) {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect.expand(4.0));
    let c = rect.center();
    let r = size / 2.0 - width / 2.0 - 2.0;
    painter.circle_stroke(c, r, Stroke::new(width, p.track));
    let mut hovered = None;
    if let Some(pos) = resp.hover_pos() {
        let d = pos - c;
        let dist = d.length();
        if (dist - r).abs() <= width / 2.0 + 2.0 {
            let mut t = d.x.atan2(-d.y) / std::f32::consts::TAU;
            if t < 0.0 {
                t += 1.0;
            }
            let mut acc = 0.0;
            for (i, (f, _)) in segments.iter().enumerate() {
                if t >= acc && t < acc + f {
                    hovered = Some(i);
                }
                acc += f;
            }
        }
    }
    let gap = 0.004;
    let mut acc = 0.0;
    for (i, (f, color)) in segments.iter().enumerate() {
        if *f > gap * 2.0 {
            let w = if hovered == Some(i) {
                width + 6.0
            } else {
                width
            };
            arc(&painter, c, r, w, acc + gap, acc + f - gap, *color, false);
        }
        acc += f;
    }
    painter.circle_stroke(c, r - width / 2.0 - 12.0, Stroke::new(1.0, p.border));
    (resp, hovered)
}

/// Round gauge with text in the middle.
pub fn gauge(ui: &mut Ui, size: f32, width: f32, fraction: f32, color: Color32, label: &str) {
    let p = pal(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect.expand(2.0));
    let c = rect.center();
    let r = size / 2.0 - width / 2.0 - 1.0;
    painter.circle_stroke(c, r, Stroke::new(width, p.track));
    arc(
        &painter,
        c,
        r,
        width,
        0.0,
        fraction.clamp(0.0, 1.0),
        color,
        true,
    );
    painter.text(
        c,
        egui::Align2::CENTER_CENTER,
        label,
        bold(size * 0.22),
        p.text,
    );
}

/// Thin rounded progress bar.
pub fn bar(ui: &mut Ui, width: f32, height: f32, fraction: f32, color: Color32) -> Response {
    let p = pal(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let r = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, r, p.track);
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * f).max(height));
        ui.painter().rect_filled(fill, r, color);
    }
    resp
}

/// Small coloured square used in legends.
pub fn swatch(ui: &mut Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(3), color);
}

/// Pill-shaped label.
pub fn pill(ui: &mut Ui, text: impl Into<String>, fg: Color32, bg: Color32) -> Response {
    let galley = ui.painter().layout_no_wrap(text.into(), bold(12.0), fg);
    let size = galley.size() + vec2(20.0, 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(255), bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

/// Filled accent button (one per view).
pub fn primary_button(ui: &mut Ui, icon: &str, text: &str) -> Response {
    let p = pal(ui);
    ui.add(
        egui::Button::new(
            RichText::new(format!("{icon}  {text}"))
                .font(bold(14.0))
                .color(p.on_accent),
        )
        .fill(p.accent)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(12))
        .min_size(vec2(0.0, 40.0)),
    )
}

/// Outlined secondary button.
pub fn secondary_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = pal(ui);
    ui.add(
        egui::Button::new(RichText::new(text.into()).color(p.text))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, p.border_hi))
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// Tinted button (e.g. danger or success actions).
pub fn tinted_button(ui: &mut Ui, text: impl Into<String>, fg: Color32, bg: Color32) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text.into()).color(fg))
            .fill(bg)
            .stroke(Stroke::new(1.0, mix(bg, fg, 0.3)))
            .corner_radius(CornerRadius::same(10))
            .min_size(vec2(0.0, 32.0)),
    )
}

/// Sidebar entry: icon, label and an optional badge on the right.
pub fn nav_item(
    ui: &mut Ui,
    icon: &str,
    label: &str,
    badge: Option<&str>,
    selected: bool,
    collapsed: bool,
) -> Response {
    let p = pal(ui);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 38.0), Sense::click());
    let resp = if collapsed {
        resp.on_hover_text(label)
    } else {
        resp
    };
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(10), p.accent_bg);
        let bar = Rect::from_min_size(rect.left_top() + vec2(0.0, 9.0), vec2(3.0, 20.0));
        painter.rect_filled(bar, CornerRadius::same(2), p.accent);
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(10), p.card_hi);
    }
    let fg = if selected { p.text } else { p.text_dim };
    let icon_color = if selected {
        p.accent_text
    } else {
        p.text_muted
    };
    if collapsed {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            FontId::proportional(19.0),
            icon_color,
        );
    } else {
        painter.text(
            rect.left_center() + vec2(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon,
            FontId::proportional(18.0),
            icon_color,
        );
        let font = if selected {
            bold(14.0)
        } else {
            FontId::proportional(14.0)
        };
        painter.text(
            rect.left_center() + vec2(42.0, 0.0),
            egui::Align2::LEFT_CENTER,
            label,
            font,
            fg,
        );
        if let Some(b) = badge {
            painter.text(
                rect.right_center() - vec2(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                b,
                FontId::proportional(11.5),
                p.text_muted,
            );
        }
    }
    resp
}

/// Small uppercase section label in the sidebar.
pub fn nav_section(ui: &mut Ui, text: &str) {
    let p = pal(ui);
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.label(
            RichText::new(text.to_uppercase())
                .font(FontId::proportional(11.0))
                .color(p.text_muted)
                .extra_letter_spacing(0.8),
        );
    });
    ui.add_space(2.0);
}

/// Stat tile: muted label above a big number.
pub fn stat_tile(ui: &mut Ui, label: &str, value: &str) {
    let p = pal(ui);
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(12.0).color(p.text_muted));
        ui.label(RichText::new(value).font(heading(26.0)).color(p.text));
    });
}
