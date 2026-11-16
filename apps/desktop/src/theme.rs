//! Drawn like the logo: one pink line of constant weight on cream (light) or plum (dark), a hollow square for every room. The same
//! tokens as the website; the room pigments are the generator's and do not change between themes.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Theme};

/// Shared 4 px spacing base, in points.
pub const GAP: f32 = 4.0;
pub const GAP_MEDIUM: f32 = 6.0;
pub const PAD: f32 = 8.0;
/// Tab bar height, in points.
pub const TAB_HEIGHT: f32 = 32.0;
pub const RADIUS: u8 = 4;
/// Weight of the pink line (rails, tab marks, node outlines), in points.
pub const LINE: f32 = 3.0;
/// Width reserved at the left of a section for its rail, and the size of the hollow square on it.
pub const RAIL: f32 = 24.0;
pub const NODE: f32 = 14.0;
pub const ON_PRIMARY: Color32 = Color32::from_rgb(0x26, 0x12, 0x1a);

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy)]
pub struct Colors {
    /// Title bar and status bar (`title_bar.background`, `status_bar.background`).
    pub bar: Color32,
    /// Panels and the tab bar (`panel.background`, `tab_bar.background`).
    pub panel: Color32,
    /// The editor, here the map (`editor.background`, `tab.active_background`).
    pub editor: Color32,
    pub element: Color32,
    pub element_hover: Color32,
    pub element_active: Color32,
    pub border: Color32,
    pub border_variant: Color32,
    pub border_focused: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    /// Pink marker fill, paired with [`ON_PRIMARY`].
    pub primary: Color32,
    pub success: Color32,
    pub error: Color32,
}

pub fn colors(dark: bool) -> Colors {
    if dark {
        Colors {
            bar: rgb(0x1c0e13),
            panel: rgb(0x24131a),
            editor: rgb(0x190c11),
            element: rgb(0x321b24),
            element_hover: rgb(0x402430),
            element_active: rgb(0x5a2a3b),
            border: rgb(0x9a7d86),
            border_variant: rgb(0x4a2f38),
            border_focused: rgb(0xff7a9c),
            text: rgb(0xfff4ec),
            text_muted: rgb(0xcdb5bc),
            accent: rgb(0xff7a9c),
            primary: rgb(0xff2e63),
            success: rgb(0x8eaf91),
            error: rgb(0xff9aa8),
        }
    } else {
        Colors {
            bar: rgb(0xf5e1d4),
            panel: rgb(0xfff4ec),
            editor: rgb(0xfffaf5),
            element: rgb(0xf7e3d6),
            element_hover: rgb(0xf0d6c6),
            element_active: rgb(0xffd0dc),
            border: rgb(0x8f746b),
            border_variant: rgb(0xecd3c4),
            border_focused: rgb(0xb3123f),
            text: rgb(0x26121a),
            text_muted: rgb(0x5a4048),
            accent: rgb(0xb3123f),
            primary: rgb(0xff2e63),
            success: rgb(0x2f5a43),
            error: rgb(0xa3162f),
        }
    }
}

/// The family of titles, tabs and the brand (Varela Round); the body keeps egui's default face.
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "varela-round".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/VarelaRound-Regular.ttf"
        ))),
    );
    let mut chain = vec!["varela-round".to_string()];
    chain.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts.families.insert(display(), chain);
    ctx.set_fonts(fonts);
}

pub fn of(ui: &egui::Ui) -> Colors {
    colors(ui.visuals().dark_mode)
}

/// A bar (title or status): the window background of the theme, a 1 px border on the side facing the content.
pub fn bar_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.bar)
        .inner_margin(Margin::symmetric(PAD as i8, GAP as i8))
}

/// A panel: the panel surface.
pub fn panel_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.panel)
        .inner_margin(Margin::symmetric(PAD as i8, GAP as i8))
}

/// A section of a panel, drawn like a room on a floor plan: a hollow square on a pink rail at the left, joined to the one above and
/// below, and the title beside it. No box: the rail is the structure.
pub fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let c = of(ui);
    let frame = egui::Frame::new().inner_margin(Margin {
        left: RAIL as i8,
        right: 0,
        top: 0,
        bottom: 0,
    });
    let out = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        let title = ui.label(
            egui::RichText::new(title)
                .font(FontId::new(17.0, display()))
                .color(c.text),
        );
        ui.add_space(GAP);
        body(ui);
        title.rect
    });
    let rect = out.response.rect;
    let x = rect.left() + (RAIL - NODE) / 2.0 + NODE / 2.0 - 3.0;
    let y = out.inner.center().y;
    let painter = ui.painter();
    // The rail runs a little past the section on both sides, so neighbouring sections are joined.
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(x - LINE / 2.0, rect.top() - GAP - 2.0),
            egui::pos2(x + LINE / 2.0, rect.bottom() + GAP + 2.0),
        ),
        CornerRadius::same((LINE / 2.0) as u8),
        c.primary,
    );
    let node = egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(NODE, NODE));
    painter.rect(
        node,
        CornerRadius::same(3),
        c.panel,
        Stroke::new(LINE, c.primary),
        egui::StrokeKind::Inside,
    );
}

/// The brand for the top bar: a hollow square (a room) and the name in the display face.
pub fn brand(ui: &mut egui::Ui) {
    let c = of(ui);
    let text = egui::RichText::new("tapiti")
        .font(FontId::new(22.0, display()))
        .color(c.text);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(NODE, NODE), egui::Sense::hover());
    ui.painter().rect(
        rect,
        CornerRadius::same(3),
        c.bar,
        Stroke::new(LINE, c.primary),
        egui::StrokeKind::Inside,
    );
    ui.label(text);
}

/// The text and surfaces of every widget, in the light and the dark variant.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    for (theme, dark) in [(Theme::Dark, true), (Theme::Light, false)] {
        let c = colors(dark);
        ctx.style_mut_of(theme, |style| {
            let radius = CornerRadius::same(RADIUS);
            let v = &mut style.visuals;
            v.panel_fill = c.panel;
            v.window_fill = c.panel;
            v.window_stroke = Stroke::new(1.0, c.border);
            v.window_corner_radius = CornerRadius::same(RADIUS + 4);
            v.extreme_bg_color = c.editor;
            v.faint_bg_color = c.element_hover;
            v.code_bg_color = c.editor;
            v.hyperlink_color = c.accent;
            v.weak_text_color = Some(c.text_muted);
            v.selection.bg_fill = c.element_active;
            v.selection.stroke = Stroke::new(1.0, c.accent);
            v.widgets.noninteractive.bg_fill = c.panel;
            v.widgets.noninteractive.weak_bg_fill = c.panel;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.border_variant);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
            for (widget, fill) in [
                (&mut v.widgets.inactive, c.element),
                (&mut v.widgets.hovered, c.element_hover),
                (&mut v.widgets.active, c.element_active),
                (&mut v.widgets.open, c.element_hover),
            ] {
                widget.bg_fill = fill;
                widget.weak_bg_fill = fill;
                widget.bg_stroke = Stroke::new(1.0, c.border);
                widget.fg_stroke = Stroke::new(1.0, c.text);
                widget.corner_radius = radius;
                widget.expansion = 0.0;
            }
            // Inputs (drag values, check boxes, text boxes) sit on the editor surface; buttons on the element surface.
            v.widgets.inactive.bg_fill = c.editor;
            v.widgets.hovered.bg_fill = c.editor;
            v.widgets.noninteractive.corner_radius = radius;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.border);
            v.widgets.active.bg_stroke = Stroke::new(1.0, c.border_focused);

            let s = &mut style.spacing;
            s.item_spacing = egui::vec2(PAD, GAP_MEDIUM);
            s.button_padding = egui::vec2(PAD, GAP);
            s.interact_size.y = 28.0;
            s.indent = 12.0;
            s.window_margin = Margin::same(PAD as i8);
            s.menu_margin = Margin::same(GAP as i8);

            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, FontId::new(20.0, display()));
            style
                .text_styles
                .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        });
    }
}

/// One tab: 32 px high, in the display face; the active one has a round pink line along its bottom edge, the others sit in muted text
/// over a 1 px rule. Returns the response for clicks.
pub fn tab(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let c = of(ui);
    let font = FontId::new(15.0, display());
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, c.text);
    let size = egui::vec2(galley.size().x + 2.0 * PAD, TAB_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() && !selected {
        painter.rect_filled(rect, CornerRadius::same(RADIUS), c.element_hover);
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, c.border_variant),
    );
    if selected {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left() + 2.0, rect.bottom() - LINE),
                egui::pos2(rect.right() - 2.0, rect.bottom()),
            ),
            CornerRadius::same((LINE / 2.0) as u8),
            c.primary,
        );
    }
    let color = if selected || response.hovered() {
        c.text
    } else {
        c.text_muted
    };
    painter.galley(
        egui::pos2(
            rect.left() + PAD,
            rect.center().y - galley.size().y / 2.0 - 1.0,
        ),
        galley,
        color,
    );
    response
}

fn preference_file() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config")))?;
    Some(base.join("tapiti").join("theme"))
}

/// The theme chosen last time (`light`, `dark` or `system`); light when none was saved.
pub fn saved_preference() -> egui::ThemePreference {
    match preference_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .as_deref()
        .map(str::trim)
    {
        Some("dark") => egui::ThemePreference::Dark,
        Some("system") => egui::ThemePreference::System,
        _ => egui::ThemePreference::Light,
    }
}

/// Changes the theme and remembers it; a file that cannot be written only means the choice is not kept.
pub fn set_preference(ctx: &egui::Context, pref: egui::ThemePreference) {
    if ctx.options(|o| o.theme_preference) == pref {
        return;
    }
    ctx.set_theme(pref);
    let name = match pref {
        egui::ThemePreference::Dark => "dark",
        egui::ThemePreference::Light => "light",
        egui::ThemePreference::System => "system",
    };
    if let Some(file) = preference_file() {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(file, name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f64 {
        [c.r(), c.g(), c.b()]
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(v, weight)| {
                let v = f64::from(v) / 255.0;
                weight
                    * if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
            })
            .sum()
    }

    #[test]
    fn both_themes_keep_text_and_controls_readable() {
        for dark in [false, true] {
            let c = colors(dark);
            let ratio = |a, b| {
                let (a, b) = (luminance(a), luminance(b));
                (a.max(b) + 0.05) / (a.min(b) + 0.05)
            };
            for surface in [
                c.bar,
                c.panel,
                c.editor,
                c.element,
                c.element_hover,
                c.element_active,
            ] {
                assert!(ratio(c.text, surface) >= 4.5);
                assert!(
                    ratio(c.text_muted, surface) >= 4.5,
                    "dark={dark}, surface={surface:?}: {}",
                    ratio(c.text_muted, surface)
                );
            }
            assert!(ratio(ON_PRIMARY, c.primary) >= 4.5);
            assert!(ratio(c.border, c.editor) >= 3.0);
            assert!(ratio(c.accent, c.panel) >= 4.5);
        }
    }
}
