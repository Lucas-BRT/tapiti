//! Bench: a workbench for a measurable thing. Graphite (dark) or cool paper (light), amber for what you changed or selected, cyan for
//! measures, the generator's own pigments for rooms. Text is at least 7:1 on every surface it sits on (the tests check it), so the
//! hierarchy comes from size, weight and position, not from grey levels. Radius 0, hairlines, no shadows.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Theme};

/// The spacing set: compact is the default, comfortable is a preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Compact,
    Comfortable,
}

impl Density {
    pub fn saved() -> Self {
        match crate::prefs::load("density").as_deref() {
            Some("comfortable") => Self::Comfortable,
            _ => Self::Compact,
        }
    }

    pub fn save(self) {
        crate::prefs::save(
            "density",
            match self {
                Self::Compact => "compact",
                Self::Comfortable => "comfortable",
            },
        );
    }

    /// Body text size, in points.
    pub fn text(self) -> f32 {
        match self {
            Self::Compact => 12.0,
            Self::Comfortable => 14.0,
        }
    }

    /// Height of a row of a control, in points.
    pub fn row(self) -> f32 {
        match self {
            Self::Compact => 20.0,
            Self::Comfortable => 28.0,
        }
    }

    fn item_spacing(self) -> egui::Vec2 {
        match self {
            Self::Compact => egui::vec2(8.0, 3.0),
            Self::Comfortable => egui::vec2(10.0, 6.0),
        }
    }

    fn button_padding(self) -> egui::Vec2 {
        match self {
            Self::Compact => egui::vec2(8.0, 2.0),
            Self::Comfortable => egui::vec2(10.0, 4.0),
        }
    }
}

/// Shared spacing, in points.
pub const GAP: f32 = 4.0;
pub const PAD: f32 = 8.0;
/// Height of the bars (menu and status, controls, command line), in points.
pub const BAR_HEIGHT: f32 = 30.0;
/// Corners are square.
pub const RADIUS: u8 = 0;
/// Text on the amber fill of a primary button.
pub const ON_PRIMARY: Color32 = Color32::from_rgb(0x0e, 0x10, 0x12);

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Clone, Copy)]
pub struct Colors {
    /// The bars: menu and status, controls and command line.
    pub bar: Color32,
    /// The side panels.
    pub panel: Color32,
    /// The canvas, and the field of a text input.
    pub editor: Color32,
    /// A group header, a button at rest.
    pub element: Color32,
    pub element_hover: Color32,
    pub element_active: Color32,
    /// The outline of a control: 3:1 or more on the surfaces it sits on.
    pub border: Color32,
    /// A hairline between rows and panels. Never carries information alone.
    pub border_variant: Color32,
    pub border_focused: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    /// Amber: what you changed, what is selected, the exit.
    pub accent: Color32,
    /// Fill of the primary action, paired with [`ON_PRIMARY`].
    pub primary: Color32,
    /// Cyan: a measure.
    pub measure: Color32,
    pub success: Color32,
    pub error: Color32,
}

pub fn colors(dark: bool) -> Colors {
    if dark {
        Colors {
            bar: rgb(0x131619),
            panel: rgb(0x131619),
            editor: rgb(0x0e1012),
            element: rgb(0x191d21),
            element_hover: rgb(0x20252a),
            element_active: rgb(0x20252a),
            border: rgb(0x6c7884),
            border_variant: rgb(0x2a3138),
            border_focused: rgb(0xffb454),
            text: rgb(0xd2d9df),
            text_muted: rgb(0xa9b0b7),
            accent: rgb(0xffb454),
            primary: rgb(0xffb454),
            measure: rgb(0x62cbe8),
            success: rgb(0x7bd88f),
            error: rgb(0xff8f8f),
        }
    } else {
        Colors {
            bar: rgb(0xe9edf0),
            panel: rgb(0xf3f5f7),
            editor: rgb(0xffffff),
            element: rgb(0xe9edf0),
            element_hover: rgb(0xdde3e8),
            element_active: rgb(0xdde3e8),
            border: rgb(0x6a7680),
            border_variant: rgb(0xcdd5db),
            border_focused: rgb(0x693e00),
            text: rgb(0x1b2127),
            text_muted: rgb(0x3f4a53),
            accent: rgb(0x693e00),
            primary: rgb(0xffb454),
            measure: rgb(0x074e62),
            success: rgb(0x175329),
            error: rgb(0x8d1b16),
        }
    }
}

/// The family of titles (IBM Plex Sans Condensed SemiBold).
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}

/// The family of values (IBM Plex Mono Medium).
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, bytes) in [
        (
            "plex-mono",
            &include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")[..],
        ),
        (
            "plex-mono-medium",
            &include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf")[..],
        ),
        (
            "plex-condensed",
            &include_bytes!("../assets/fonts/IBMPlexSansCondensed-SemiBold.ttf")[..],
        ),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    let fallback = |fonts: &egui::FontDefinitions, family: FontFamily| {
        fonts.families.get(&family).cloned().unwrap_or_default()
    };
    let prop = fallback(&fonts, FontFamily::Proportional);
    let mono = fallback(&fonts, FontFamily::Monospace);
    // Everything is set in Plex Mono; egui's own faces stay behind it for the glyphs Plex does not have.
    let chain = |first: &str, rest: &[String]| {
        let mut v = vec![first.to_string()];
        v.extend(rest.iter().cloned());
        v
    };
    fonts
        .families
        .insert(FontFamily::Proportional, chain("plex-mono", &prop));
    fonts
        .families
        .insert(FontFamily::Monospace, chain("plex-mono", &mono));
    fonts
        .families
        .insert(medium(), chain("plex-mono-medium", &prop));
    fonts
        .families
        .insert(display(), chain("plex-condensed", &prop));
    ctx.set_fonts(fonts);
}

pub fn of(ui: &egui::Ui) -> Colors {
    colors(ui.visuals().dark_mode)
}

/// A bar: the bar surface and a hairline on the side facing the content.
pub fn bar_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.bar)
        .inner_margin(Margin::symmetric(PAD as i8, 0))
}

/// A side panel: the panel surface.
pub fn panel_frame(ui: &egui::Ui) -> egui::Frame {
    let c = of(ui);
    egui::Frame::new().fill(c.panel)
}

/// A group of a panel: a raised header bar with the title in the display face, and the body under it with a hairline at the foot.
pub fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let c = of(ui);
    egui::Frame::new()
        .fill(c.element)
        .stroke(Stroke::new(1.0, c.border_variant))
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .font(FontId::new(14.0, display()))
                    .color(c.text),
            );
        });
    egui::Frame::new()
        .inner_margin(Margin {
            left: 10,
            right: 8,
            top: 6,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            set_body_width(ui);
            body(ui);
        });
    let y = ui.cursor().top();
    let r = ui.max_rect();
    ui.painter()
        .hline(r.x_range(), y, Stroke::new(1.0, c.border_variant));
}

/// The width of the body of the group or section being drawn. A grid cell's own `available_width` shrinks to what the previous frame used,
/// so rows that size their label from it get narrower at every frame; they read this instead.
pub fn body_width(ui: &egui::Ui) -> f32 {
    ui.data(|d| d.get_temp::<f32>(egui::Id::NULL.with("tapiti-body-width")))
        .unwrap_or_else(|| ui.available_width())
}

fn set_body_width(ui: &egui::Ui) {
    let w = ui.available_width();
    ui.data_mut(|d| d.insert_temp(egui::Id::NULL.with("tapiti-body-width"), w));
}

/// The ids of the two text fields the keyboard can jump to: the filter of the tree and the command line.
pub fn filter_id() -> egui::Id {
    egui::Id::NULL.with("tapiti-filter")
}

pub fn command_id() -> egui::Id {
    egui::Id::NULL.with("tapiti-command")
}

/// What a group bar says: its title, how many settings it holds, how many of them differ from the defaults, and whether it starts open.
pub struct GroupBar<'a> {
    pub title: &'a str,
    pub count: usize,
    pub changed: usize,
    pub default_open: bool,
}

/// A collapsible group of the tree: a bar with a caret, the title in the display face, the count of changed settings in amber and the
/// number of settings on the right; the body opens under it. `force` opens or closes it from outside (a filter match, a focus). The
/// returned response is the bar's, to scroll to.
pub fn group(
    ui: &mut egui::Ui,
    id: egui::Id,
    bar: GroupBar,
    force: Option<bool>,
    body: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let GroupBar {
        title,
        count,
        changed,
        default_open,
    } = bar;
    use egui::collapsing_header::CollapsingState;
    let c = of(ui);
    let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    if let Some(open) = force {
        state.set_open(open);
    }
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::click());
    if response.clicked() {
        state.toggle(ui);
    }
    let open = state.is_open();
    let painter = ui.painter();
    let fill = if response.hovered() {
        c.element_hover
    } else {
        c.element
    };
    painter.rect_filled(rect, 0.0, fill);
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, c.border_variant),
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            0.0,
            Stroke::new(1.0, c.border_focused),
            egui::StrokeKind::Inside,
        );
    }
    let mid = rect.center().y;
    // The caret is drawn: Plex Mono has no triangle glyphs.
    let (cx, h) = (rect.left() + 14.0, 4.0);
    let caret = if open {
        vec![
            egui::pos2(cx - h, mid - 2.0),
            egui::pos2(cx + h, mid - 2.0),
            egui::pos2(cx, mid + 3.0),
        ]
    } else {
        vec![
            egui::pos2(cx - 2.0, mid - h),
            egui::pos2(cx - 2.0, mid + h),
            egui::pos2(cx + 3.0, mid),
        ]
    };
    painter.add(egui::Shape::convex_polygon(
        caret,
        c.text_muted,
        Stroke::NONE,
    ));
    painter.text(
        egui::pos2(rect.left() + 28.0, mid),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::new(14.0, display()),
        c.text,
    );
    let count = painter.text(
        egui::pos2(rect.right() - 10.0, mid),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        FontId::proportional(11.0),
        c.text_muted,
    );
    if changed > 0 {
        painter.text(
            egui::pos2(count.left() - 10.0, mid),
            egui::Align2::RIGHT_CENTER,
            format!("{changed} changed"),
            FontId::new(11.0, medium()),
            c.accent,
        );
    }
    state.show_body_unindented(ui, |ui| {
        egui::Frame::new()
            .inner_margin(Margin {
                left: 10,
                right: 8,
                top: 6,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                set_body_width(ui);
                body(ui);
            });
    });
    response
}

/// Plain text inside a column, with the same margin as the groups' bodies.
pub fn note(ui: &mut egui::Ui, text: egui::RichText) {
    egui::Frame::new()
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(text);
        });
}

/// The brand for the menu bar: an amber square and the name in the display face.
pub fn brand(ui: &mut egui::Ui) {
    let c = of(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, c.accent);
    ui.label(
        egui::RichText::new("tapiti")
            .font(FontId::new(15.0, display()))
            .color(c.text),
    );
}

/// The text and surfaces of every widget, in the light and the dark variant, at the density in use.
pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    apply_density(ctx, Density::saved());
}

pub fn apply_density(ctx: &egui::Context, density: Density) {
    for (theme, dark) in [(Theme::Dark, true), (Theme::Light, false)] {
        let c = colors(dark);
        ctx.style_mut_of(theme, |style| {
            let radius = CornerRadius::same(RADIUS);
            let v = &mut style.visuals;
            v.panel_fill = c.panel;
            v.window_fill = c.panel;
            v.window_stroke = Stroke::new(1.0, c.border);
            v.window_corner_radius = CornerRadius::same(RADIUS);
            v.menu_corner_radius = CornerRadius::same(RADIUS);
            v.window_shadow = egui::Shadow::NONE;
            v.popup_shadow = egui::Shadow::NONE;
            v.extreme_bg_color = c.editor;
            v.faint_bg_color = c.element;
            v.code_bg_color = c.editor;
            v.hyperlink_color = c.measure;
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
            // Inputs (drag values, check boxes, text boxes) sit on the canvas surface; buttons on the element surface.
            v.widgets.inactive.bg_fill = c.editor;
            v.widgets.hovered.bg_fill = c.editor;
            v.widgets.noninteractive.corner_radius = radius;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, c.text_muted);
            v.widgets.active.bg_stroke = Stroke::new(1.0, c.border_focused);

            let s = &mut style.spacing;
            s.item_spacing = density.item_spacing();
            s.button_padding = density.button_padding();
            s.interact_size.y = density.row();
            s.indent = 12.0;
            s.window_margin = Margin::same(PAD as i8);
            s.menu_margin = Margin::same(GAP as i8);

            let text = density.text();
            style
                .text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(text - 1.0));
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(text));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(text));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, FontId::new(text + 3.0, display()));
            style
                .text_styles
                .insert(egui::TextStyle::Monospace, FontId::monospace(text));
        });
    }
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

    fn ratio(a: Color32, b: Color32) -> f64 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// Every surface a piece of text can sit on.
    fn surfaces(c: &Colors) -> [Color32; 6] {
        [
            c.bar,
            c.panel,
            c.editor,
            c.element,
            c.element_hover,
            c.element_active,
        ]
    }

    #[test]
    fn all_text_is_seven_to_one_on_every_surface_in_both_themes() {
        for dark in [false, true] {
            let c = colors(dark);
            for (name, text) in [
                ("text", c.text),
                ("text_muted", c.text_muted),
                ("accent", c.accent),
                ("measure", c.measure),
                ("success", c.success),
                ("error", c.error),
            ] {
                for surface in surfaces(&c) {
                    let r = ratio(text, surface);
                    assert!(r >= 7.0, "dark={dark} {name} on {surface:?}: {r:.2}");
                }
            }
        }
    }

    #[test]
    fn controls_and_the_primary_action_keep_their_contrast() {
        for dark in [false, true] {
            let c = colors(dark);
            for surface in [c.panel, c.editor, c.bar] {
                assert!(
                    ratio(c.border, surface) >= 3.0,
                    "dark={dark} border on {surface:?}"
                );
                assert!(
                    ratio(c.border_focused, surface) >= 3.0,
                    "dark={dark} focus on {surface:?}"
                );
            }
            assert!(ratio(ON_PRIMARY, c.primary) >= 7.0, "dark={dark} primary");
        }
    }

    #[test]
    fn both_densities_are_defined_and_ordered() {
        assert!(Density::Compact.text() < Density::Comfortable.text());
        assert!(Density::Compact.row() < Density::Comfortable.row());
        assert!(Density::Compact.row() >= 20.0 && Density::Comfortable.row() >= 24.0);
    }
}
