//! The settings window: the options of the application itself (how it looks and moves), kept out of the main window. What shapes a floor
//! is not here; that is the parameters.

use crate::{prefs, theme, App};
use eframe::egui::{
    self, RichText, Ui, ViewportBuilder, ViewportClass, ViewportCommand, ViewportId,
};

/// How the reading panel on the right is shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reading {
    /// Shown when the window is wide enough.
    Automatic,
    Shown,
    Hidden,
}

impl Reading {
    pub fn saved() -> Self {
        match prefs::load("reading").as_deref() {
            Some("shown") => Self::Shown,
            Some("hidden") => Self::Hidden,
            _ => Self::Automatic,
        }
    }

    fn save(self) {
        prefs::save(
            "reading",
            match self {
                Self::Automatic => "automatic",
                Self::Shown => "shown",
                Self::Hidden => "hidden",
            },
        );
    }

    /// The `App::reading_open` that stands for this choice.
    pub fn open(self) -> Option<bool> {
        match self {
            Self::Automatic => None,
            Self::Shown => Some(true),
            Self::Hidden => Some(false),
        }
    }

    pub fn of(open: Option<bool>) -> Self {
        match open {
            None => Self::Automatic,
            Some(true) => Self::Shown,
            Some(false) => Self::Hidden,
        }
    }
}

/// Shows the window while `app.show_settings` is set, as a window of its own.
pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.show_settings {
        return;
    }
    let builder = ViewportBuilder::default()
        .with_title("Tapiti settings")
        .with_app_id(crate::APP_ID)
        .with_inner_size([480.0, 420.0])
        .with_min_inner_size([380.0, 320.0]);
    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("tapiti-settings"),
        builder,
        |ui, class| {
            if class == ViewportClass::EmbeddedWindow {
                // The backend has no second window: show it in the first one.
                let mut open = true;
                egui::Window::new("Settings")
                    .open(&mut open)
                    .collapsible(false)
                    .show(ui.ctx(), |ui| content(app, ui));
                app.show_settings = open;
                return;
            }
            egui::CentralPanel::default()
                .frame(theme::panel_frame(ui))
                .show(ui, |ui| content(app, ui));
            let close = ui
                .ctx()
                .input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));
            if close {
                app.show_settings = false;
                ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            }
        },
    );
}

/// One of the options of the application that is a choice among presets, one of which may be a value of the user's own.
trait Choice: Copy + PartialEq {
    const LABELS: &'static [&'static str];
    fn index(self) -> usize;
    /// The choice of preset `i`; the custom one starts from the value in use.
    fn pick(self, i: usize) -> Self;
    /// The value of the user's own, while that preset is chosen.
    fn custom(self) -> Option<Custom>;
    fn with_custom(self, value: f32) -> Self;
}

/// A value the user sets, and the range it may take.
struct Custom {
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &'static str,
}

/// The name of an option, its presets side by side, and the slider of the custom one underneath when it is chosen. True when it changed.
fn choice_row<C: Choice>(ui: &mut Ui, name: &str, help: &str, value: &mut C) -> bool {
    let before = *value;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(name).font(egui::FontId::new(13.0, theme::medium())));
        ui.add_space(8.0);
        for (i, label) in C::LABELS.iter().enumerate() {
            if ui.selectable_label(value.index() == i, *label).clicked() {
                *value = value.pick(i);
            }
        }
    });
    if let Some(custom) = value.custom() {
        let mut v = custom.value;
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            // The rail of a slider is drawn in the colour of an idle control, which is nearly the panel's: give it the border's.
            let c = theme::of(ui);
            ui.visuals_mut().widgets.inactive.bg_fill = c.border;
            ui.spacing_mut().slider_width = 220.0;
            ui.add(
                egui::Slider::new(&mut v, custom.range.clone())
                    .suffix(custom.suffix)
                    .trailing_fill(true)
                    .integer(),
            );
        });
        *value = value.with_custom(v);
    }
    ui.label(
        RichText::new(help)
            .color(theme::of(ui).text_muted)
            .size(12.0),
    );
    ui.add_space(6.0);
    *value != before
}

/// How long a motion lasts: off, one of three speeds, or a time of the user's own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Speed {
    Off,
    Fast,
    Normal,
    Slow,
    /// Milliseconds.
    Custom(u16),
}

/// What a `Speed` times: the interface's own transitions (a panel sliding, a group opening, a control changing colour) or the reveal of a
/// new floor. Each has its own times for the three speeds, its own range and its own file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Timing {
    pub speed: Speed,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Interface,
    Reveal,
}

impl Kind {
    fn file(self) -> &'static str {
        match self {
            Self::Interface => "ui_motion",
            Self::Reveal => "reveal_motion",
        }
    }

    /// Fast, normal and slow, in milliseconds.
    fn table(self) -> [u16; 3] {
        match self {
            Self::Interface => [80, 120, 220],
            Self::Reveal => [300, 600, 1200],
        }
    }

    fn range(self) -> std::ops::RangeInclusive<f32> {
        match self {
            Self::Interface => 0.0..=600.0,
            Self::Reveal => 0.0..=3000.0,
        }
    }
}

impl Timing {
    pub fn interface() -> Self {
        Self::load(Kind::Interface)
    }

    pub fn reveal() -> Self {
        Self::load(Kind::Reveal)
    }

    fn load(kind: Kind) -> Self {
        let speed = match prefs::load(kind.file()).as_deref() {
            Some("off") => Speed::Off,
            Some("fast") => Speed::Fast,
            Some("slow") => Speed::Slow,
            Some(other) if other.starts_with("custom:") => other[7..]
                .parse()
                .map_or(Speed::Normal, |ms: u16| Speed::Custom(ms.min(10_000))),
            Some(_) => Speed::Normal,
            // The files of the first version: one switch for all motion, and one for the reveal.
            None if prefs::load("motion").as_deref() == Some("reduce") => Speed::Off,
            None if kind == Kind::Reveal && prefs::load("reveal").as_deref() == Some("off") => {
                Speed::Off
            }
            None => Speed::Normal,
        };
        Self { speed, kind }
    }

    pub fn save(self) {
        prefs::save(
            self.kind.file(),
            &match self.speed {
                Speed::Off => "off".to_string(),
                Speed::Fast => "fast".to_string(),
                Speed::Normal => "normal".to_string(),
                Speed::Slow => "slow".to_string(),
                Speed::Custom(ms) => format!("custom:{ms}"),
            },
        );
    }

    pub fn ms(self) -> u16 {
        let [fast, normal, slow] = self.kind.table();
        match self.speed {
            Speed::Off => 0,
            Speed::Fast => fast,
            Speed::Normal => normal,
            Speed::Slow => slow,
            Speed::Custom(ms) => ms,
        }
    }

    pub fn seconds(self) -> f32 {
        f32::from(self.ms()) / 1000.0
    }

    pub fn is_on(self) -> bool {
        self.ms() > 0
    }
}

impl Choice for Timing {
    const LABELS: &'static [&'static str] = &["Off", "Fast", "Normal", "Slow", "Custom"];

    fn index(self) -> usize {
        match self.speed {
            Speed::Off => 0,
            Speed::Fast => 1,
            Speed::Normal => 2,
            Speed::Slow => 3,
            Speed::Custom(_) => 4,
        }
    }

    fn pick(self, i: usize) -> Self {
        let speed = match i {
            0 => Speed::Off,
            1 => Speed::Fast,
            2 => Speed::Normal,
            3 => Speed::Slow,
            _ => Speed::Custom(self.ms()),
        };
        Self { speed, ..self }
    }

    fn custom(self) -> Option<Custom> {
        matches!(self.speed, Speed::Custom(_)).then(|| Custom {
            value: f32::from(self.ms()),
            range: self.kind.range(),
            suffix: " ms",
        })
    }

    fn with_custom(self, value: f32) -> Self {
        Self {
            speed: Speed::Custom(value as u16),
            ..self
        }
    }
}

/// The size of everything in the window, as a scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextScale {
    Small,
    Normal,
    Large,
    Larger,
    /// Percent.
    Custom(u16),
}

impl TextScale {
    pub fn saved() -> Self {
        match prefs::load("text").as_deref() {
            Some("small") => Self::Small,
            Some("large") => Self::Large,
            Some("larger") => Self::Larger,
            Some(other) if other.starts_with("custom:") => other[7..]
                .parse()
                .map_or(Self::Normal, |p: u16| Self::Custom(p.clamp(70, 200))),
            _ => Self::Normal,
        }
    }

    pub fn save(self) {
        prefs::save(
            "text",
            &match self {
                Self::Small => "small".to_string(),
                Self::Normal => "normal".to_string(),
                Self::Large => "large".to_string(),
                Self::Larger => "larger".to_string(),
                Self::Custom(p) => format!("custom:{p}"),
            },
        );
    }

    pub fn percent(self) -> u16 {
        match self {
            Self::Small => 90,
            Self::Normal => 100,
            Self::Large => 120,
            Self::Larger => 140,
            Self::Custom(p) => p,
        }
    }

    pub fn factor(self) -> f32 {
        f32::from(self.percent()) / 100.0
    }
}

impl Choice for TextScale {
    const LABELS: &'static [&'static str] = &["Small", "Normal", "Large", "Larger", "Custom"];

    fn index(self) -> usize {
        match self {
            Self::Small => 0,
            Self::Normal => 1,
            Self::Large => 2,
            Self::Larger => 3,
            Self::Custom(_) => 4,
        }
    }

    fn pick(self, i: usize) -> Self {
        match i {
            0 => Self::Small,
            1 => Self::Normal,
            2 => Self::Large,
            3 => Self::Larger,
            _ => Self::Custom(self.percent()),
        }
    }

    fn custom(self) -> Option<Custom> {
        matches!(self, Self::Custom(_)).then(|| Custom {
            value: f32::from(self.percent()),
            range: 70.0..=200.0,
            suffix: " %",
        })
    }

    fn with_custom(self, value: f32) -> Self {
        Self::Custom(value as u16)
    }
}

impl Choice for theme::Density {
    const LABELS: &'static [&'static str] = &["Compact", "Comfortable", "Custom"];

    fn index(self) -> usize {
        match self {
            Self::Compact => 0,
            Self::Comfortable => 1,
            Self::Custom(_) => 2,
        }
    }

    fn pick(self, i: usize) -> Self {
        match i {
            0 => Self::Compact,
            1 => Self::Comfortable,
            _ => Self::Custom(self.row() as u8),
        }
    }

    fn custom(self) -> Option<Custom> {
        matches!(self, Self::Custom(_)).then(|| Custom {
            value: self.row(),
            range: f32::from(*Self::ROW_RANGE.start())..=f32::from(*Self::ROW_RANGE.end()),
            suffix: " px a row",
        })
    }

    fn with_custom(self, value: f32) -> Self {
        Self::Custom(value as u8)
    }
}

impl Choice for theme::Appearance {
    const LABELS: &'static [&'static str] = &["System", "Light", "Dark"];

    fn index(self) -> usize {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    fn pick(self, i: usize) -> Self {
        [Self::System, Self::Light, Self::Dark][i.min(2)]
    }

    fn custom(self) -> Option<Custom> {
        None
    }

    fn with_custom(self, _: f32) -> Self {
        self
    }
}

/// The share of the window the reading panel takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReadingWidth {
    Narrow,
    Normal,
    Wide,
    /// Percent of the window.
    Custom(u8),
}

impl ReadingWidth {
    pub fn saved() -> Self {
        match prefs::load("reading_width").as_deref() {
            Some("narrow") => Self::Narrow,
            Some("wide") => Self::Wide,
            Some(other) if other.starts_with("custom:") => other[7..]
                .parse()
                .map_or(Self::Normal, |p: u8| Self::Custom(p.clamp(15, 50))),
            _ => Self::Normal,
        }
    }

    pub fn save(self) {
        prefs::save(
            "reading_width",
            &match self {
                Self::Narrow => "narrow".to_string(),
                Self::Normal => "normal".to_string(),
                Self::Wide => "wide".to_string(),
                Self::Custom(p) => format!("custom:{p}"),
            },
        );
    }

    pub fn percent(self) -> u8 {
        match self {
            Self::Narrow => 22,
            Self::Normal => 27,
            Self::Wide => 34,
            Self::Custom(p) => p,
        }
    }
}

impl Choice for ReadingWidth {
    const LABELS: &'static [&'static str] = &["Narrow", "Normal", "Wide", "Custom"];

    fn index(self) -> usize {
        match self {
            Self::Narrow => 0,
            Self::Normal => 1,
            Self::Wide => 2,
            Self::Custom(_) => 3,
        }
    }

    fn pick(self, i: usize) -> Self {
        match i {
            0 => Self::Narrow,
            1 => Self::Normal,
            2 => Self::Wide,
            _ => Self::Custom(self.percent()),
        }
    }

    fn custom(self) -> Option<Custom> {
        matches!(self, Self::Custom(_)).then(|| Custom {
            value: f32::from(self.percent()),
            range: 15.0..=50.0,
            suffix: " % of the window",
        })
    }

    fn with_custom(self, value: f32) -> Self {
        Self::Custom(value as u8)
    }
}

impl Choice for Reading {
    const LABELS: &'static [&'static str] = &["Automatic", "Shown", "Hidden"];

    fn index(self) -> usize {
        match self {
            Self::Automatic => 0,
            Self::Shown => 1,
            Self::Hidden => 2,
        }
    }

    fn pick(self, i: usize) -> Self {
        [Self::Automatic, Self::Shown, Self::Hidden][i.min(2)]
    }

    fn custom(self) -> Option<Custom> {
        None
    }

    fn with_custom(self, _: f32) -> Self {
        self
    }
}

fn content(app: &mut App, ui: &mut Ui) {
    if app.text_scale_pending && !ui.ctx().input(|i| i.pointer.any_down()) {
        ui.ctx().set_zoom_factor(app.text_scale.factor());
        app.text_scale_pending = false;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        theme::section(ui, "Appearance", |ui| {
            if choice_row(
                ui,
                "Theme",
                "System follows the desktop's light or dark scheme.",
                &mut app.appearance,
            ) {
                app.appearance.save();
            }
            if choice_row(
                ui,
                "Spacing",
                "Compact fits more parameters on the screen. Custom sets the height of a row; the text and the gaps follow it.",
                &mut app.density,
            ) {
                app.density.save();
                theme::apply_density(ui.ctx(), app.density);
            }
            if choice_row(
                ui,
                "Text size",
                "Scales the text and the controls together, the map included.",
                &mut app.text_scale,
            ) {
                app.text_scale.save();
                // Dragging the slider would rescale the window under the pointer; the size applies when the pointer is let go.
                if !ui.ctx().input(|i| i.pointer.any_down()) {
                    ui.ctx().set_zoom_factor(app.text_scale.factor());
                } else {
                    app.text_scale_pending = true;
                }
            }
        });
        theme::section(ui, "Reading panel", |ui| {
            let mut reading = Reading::of(app.reading_open);
            if choice_row(
                ui,
                "Show",
                "The rules, measures and phases of the floor, on the right. Automatic shows it when the window is wide enough, about 1180 px.",
                &mut reading,
            ) {
                app.reading_open = reading.open();
                reading.save();
            }
            if choice_row(
                ui,
                "Width",
                "The share of the window the panel takes.",
                &mut app.reading_width,
            ) {
                app.reading_width.save();
            }
        });
        theme::section(ui, "Motion", |ui| {
            if choice_row(
                ui,
                "Interface",
                "Panels sliding, groups opening and controls changing colour. Off: everything changes at once.",
                &mut app.ui_motion,
            ) {
                app.ui_motion.save();
                theme::apply_motion(ui.ctx(), app.ui_motion.seconds());
            }
            if choice_row(
                ui,
                "New floor",
                "A new floor is drawn from the entrance outwards. Stepping through seeds quickly shows each floor at once.",
                &mut app.reveal_motion,
            ) {
                app.reveal_motion.save();
            }
        });
        theme::note(
            ui,
            RichText::new(format!(
                "Saved in {} and applied at once.",
                prefs::dir().map_or_else(|| "the config folder".into(), |d| d.display().to_string())
            ))
                .color(theme::of(ui).text_muted),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timings_have_their_own_tables_and_a_custom_value() {
        let ui = Timing {
            speed: Speed::Normal,
            kind: Kind::Interface,
        };
        let reveal = Timing {
            speed: Speed::Normal,
            kind: Kind::Reveal,
        };
        assert_eq!((ui.ms(), reveal.ms()), (120, 600));
        assert_eq!(ui.pick(0).ms(), 0);
        assert!(!ui.pick(0).is_on());
        // Choosing Custom starts from the value in use, and the slider then sets it.
        let custom = reveal.pick(4);
        assert_eq!(custom.ms(), 600);
        assert_eq!(custom.with_custom(450.0).ms(), 450);
        assert_eq!(custom.custom().map(|c| c.value), Some(600.0));
    }

    #[test]
    fn scales_and_widths_have_presets_and_a_custom_value() {
        assert_eq!(TextScale::Normal.pick(2).percent(), 120);
        assert_eq!(TextScale::Large.pick(4), TextScale::Custom(120));
        assert_eq!(ReadingWidth::Wide.pick(3), ReadingWidth::Custom(34));
        assert!(ReadingWidth::Narrow.percent() < ReadingWidth::Wide.percent());
        let custom = theme::Density::Comfortable.pick(2);
        assert_eq!(custom, theme::Density::Custom(28));
        assert!(theme::Density::Compact.text() < theme::Density::Custom(34).text());
    }

    #[test]
    fn reading_choices_round_trip() {
        for r in [Reading::Automatic, Reading::Shown, Reading::Hidden] {
            assert_eq!(Reading::of(r.open()), r);
        }
    }
}
