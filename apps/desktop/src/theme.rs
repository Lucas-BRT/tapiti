//! Bench: a workbench for a measurable thing. Graphite (dark) or cool paper (light), amber for what you changed or selected, cyan for
//! measures, the generator's own pigments for rooms. Text is at least 7:1 on every surface it sits on (the tests check it), so the
//! hierarchy comes from size, weight and position, not from grey levels. Radius 0, hairlines, no shadows.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Theme};

/// The spacing set: compact is the default; comfortable and a row height of one's own are preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Compact,
    Comfortable,
    /// The height of a row of a control, in points; everything else scales with it.
    Custom(u8),
}

impl Density {
    pub const ROW_RANGE: std::ops::RangeInclusive<u8> = 18..=40;

    pub fn saved() -> Self {
        match crate::prefs::load("density").as_deref() {
            Some("comfortable") => Self::Comfortable,
            Some(other) => other
                .strip_prefix("custom:")
                .and_then(|n| n.parse::<u8>().ok())
                .map_or(Self::Compact, |n| {
                    Self::Custom(n.clamp(*Self::ROW_RANGE.start(), *Self::ROW_RANGE.end()))
                }),
            None => Self::Compact,
        }
    }

    pub fn save(self) {
        crate::prefs::save(
            "density",
            &match self {
                Self::Compact => "compact".to_string(),
                Self::Comfortable => "comfortable".to_string(),
                Self::Custom(row) => format!("custom:{row}"),
            },
        );
    }

    /// Height of a row of a control, in points.
    pub fn row(self) -> f32 {
        match self {
            Self::Compact => 20.0,
            Self::Comfortable => 28.0,
            Self::Custom(row) => f32::from(row),
        }
    }

    /// Where the row height sits between compact (0) and comfortable (1); the other sizes follow it.
    fn t(self) -> f32 {
        (self.row() - 20.0) / 8.0
    }

    fn lerp(self, compact: f32, comfortable: f32) -> f32 {
        compact + (comfortable - compact) * self.t()
    }

    /// Body text size, in points.
    pub fn text(self) -> f32 {
        self.lerp(12.0, 14.0).clamp(11.0, 18.0)
    }

    fn item_spacing(self) -> egui::Vec2 {
        egui::vec2(self.lerp(8.0, 10.0).max(6.0), self.lerp(3.0, 6.0).max(2.0))
    }

    fn button_padding(self) -> egui::Vec2 {
        egui::vec2(self.lerp(8.0, 10.0).max(6.0), self.lerp(2.0, 4.0).max(1.0))
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
    apply_motion(ctx, crate::settings::Timing::interface().seconds());
    ctx.set_zoom_factor(crate::settings::TextScale::saved().factor());
}

/// How long the interface's own transitions last (a panel sliding, a group opening, a hover fading in), in seconds; 0 for none.
pub fn apply_motion(ctx: &egui::Context, seconds: f32) {
    for theme in [Theme::Dark, Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            style.animation_time = seconds;
        });
    }
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

/// How the window is dressed: in the light theme, in the dark theme, or as the system says. Following the system is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    /// The choice kept in `~/.config/tapiti/theme`; the system's when there is none.
    pub fn saved() -> Self {
        match crate::prefs::load("theme").as_deref() {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }

    /// The next choice when the button is pressed: system, light, dark, and back to the system.
    pub fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    pub fn save(self) {
        crate::prefs::save(
            "theme",
            match self {
                Self::System => "system",
                Self::Light => "light",
                Self::Dark => "dark",
            },
        );
    }
}

/// What the system says its colour scheme is, read from the desktop portal (`org.freedesktop.appearance`), which GNOME, KDE and COSMIC
/// all answer. egui cannot ask: on Wayland the window toolkit does not report it, so "follow the system" would never change. `None`
/// when the system has no preference or cannot be asked (no `busctl`, no portal, not Linux).
#[cfg(target_os = "linux")]
pub fn system_dark() -> Option<bool> {
    let out = std::process::Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            "Read",
            "ss",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| parse_color_scheme(&String::from_utf8_lossy(&out.stdout)))
        .flatten()
}

#[cfg(not(target_os = "linux"))]
pub fn system_dark() -> Option<bool> {
    None
}

/// The answer of the portal is `v v u 1` (dark), `v v u 2` (light) or `v v u 0` (no preference): the number at the end.
fn parse_color_scheme(answer: &str) -> Option<bool> {
    match answer.split_whitespace().last()? {
        "1" => Some(true),
        "2" => Some(false),
        _ => None,
    }
}

/// The system's colour scheme, kept up to date by a thread that asks again every two seconds and wakes the window when it changes.
#[derive(Clone)]
pub struct SystemTheme(std::sync::Arc<std::sync::atomic::AtomicU8>);

impl SystemTheme {
    const UNKNOWN: u8 = 0;
    const LIGHT: u8 = 1;
    const DARK: u8 = 2;

    fn code(dark: Option<bool>) -> u8 {
        match dark {
            None => Self::UNKNOWN,
            Some(false) => Self::LIGHT,
            Some(true) => Self::DARK,
        }
    }

    /// Asks once now, so the first frame is already in the right theme, then keeps asking in the background.
    pub fn watch(ctx: &egui::Context) -> Self {
        use std::sync::atomic::Ordering::Relaxed;
        let value =
            std::sync::Arc::new(std::sync::atomic::AtomicU8::new(Self::code(system_dark())));
        let (shared, ctx) = (value.clone(), ctx.clone());
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            let now = Self::code(system_dark());
            if shared.swap(now, Relaxed) != now {
                ctx.request_repaint();
            }
        });
        Self(value)
    }

    pub fn dark(&self) -> Option<bool> {
        match self.0.load(std::sync::atomic::Ordering::Relaxed) {
            Self::LIGHT => Some(false),
            Self::DARK => Some(true),
            _ => None,
        }
    }
}

/// Puts the window in the theme `appearance` asks for; cheap enough to call every frame. When the system cannot be asked, egui's own
/// guess at the system theme is used.
pub fn apply_appearance(ctx: &egui::Context, appearance: Appearance, system: &SystemTheme) {
    let wanted = match appearance {
        Appearance::Light => egui::ThemePreference::Light,
        Appearance::Dark => egui::ThemePreference::Dark,
        Appearance::System => match system.dark() {
            Some(true) => egui::ThemePreference::Dark,
            Some(false) => egui::ThemePreference::Light,
            None => egui::ThemePreference::System,
        },
    };
    if ctx.options(|o| o.theme_preference) != wanted {
        ctx.set_theme(wanted);
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
    fn the_portal_answer_is_read() {
        assert_eq!(parse_color_scheme("v v u 1\n"), Some(true));
        assert_eq!(parse_color_scheme("v v u 2\n"), Some(false));
        assert_eq!(parse_color_scheme("v v u 0\n"), None);
        assert_eq!(parse_color_scheme(""), None);
        assert_eq!(parse_color_scheme("Failed to call method"), None);
    }

    #[test]
    fn both_densities_are_defined_and_ordered() {
        assert!(Density::Compact.text() < Density::Comfortable.text());
        assert!(Density::Compact.row() < Density::Comfortable.row());
        assert!(Density::Compact.row() >= 20.0 && Density::Comfortable.row() >= 24.0);
    }
}
