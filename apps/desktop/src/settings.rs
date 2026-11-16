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

fn content(app: &mut App, ui: &mut Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        theme::section(ui, "Appearance", |ui| {
            row(ui, "Theme", "System follows the desktop's light or dark scheme.", |ui| {
                let before = app.appearance;
                for (value, name) in [
                    (theme::Appearance::System, "System"),
                    (theme::Appearance::Light, "Light"),
                    (theme::Appearance::Dark, "Dark"),
                ] {
                    ui.selectable_value(&mut app.appearance, value, name);
                }
                if app.appearance != before {
                    app.appearance.save();
                }
            });
            row(ui, "Spacing", "Compact fits more parameters on the screen.", |ui| {
                let before = app.density;
                for (value, name) in [
                    (theme::Density::Compact, "Compact"),
                    (theme::Density::Comfortable, "Comfortable"),
                ] {
                    ui.selectable_value(&mut app.density, value, name);
                }
                if app.density != before {
                    app.density.save();
                    theme::apply_density(ui.ctx(), app.density);
                }
            });
        });
        theme::section(ui, "Window", |ui| {
            row(
                ui,
                "Reading panel",
                "The panel on the right: rules, measures and phases of the floor. Automatic shows it when the window is at least 1180 points wide.",
                |ui| {
                    let mut reading = Reading::of(app.reading_open);
                    let before = reading;
                    for (value, name) in [
                        (Reading::Automatic, "Automatic"),
                        (Reading::Shown, "Shown"),
                        (Reading::Hidden, "Hidden"),
                    ] {
                        ui.selectable_value(&mut reading, value, name);
                    }
                    if reading != before {
                        app.reading_open = reading.open();
                        reading.save();
                    }
                },
            );
        });
        theme::section(ui, "Motion", |ui| {
            row(
                ui,
                "Animations",
                "Groups open and close, and controls change colour, over 120 ms. Off: everything changes at once.",
                |ui| {
                    if ui.checkbox(&mut app.animations, "On").changed() {
                        prefs::save("motion", if app.animations { "full" } else { "reduce" });
                        theme::apply_motion(ui.ctx(), app.animations);
                    }
                },
            );
            row(
                ui,
                "New floor",
                "A new floor is drawn from the entrance outwards in about 0.6 s. Stepping through seeds quickly shows each floor at once.",
                |ui| {
                    ui.add_enabled_ui(app.animations, |ui| {
                        if ui
                            .checkbox(&mut app.reveal_floor, "Reveal from the entrance")
                            .changed()
                        {
                            prefs::save("reveal", if app.reveal_floor { "on" } else { "off" });
                        }
                    });
                },
            );
        });
        theme::note(
            ui,
            RichText::new("Saved in ~/.config/tapiti and applied at once.")
                .color(theme::of(ui).text_muted),
        );
    });
}

/// A name, its control on the same line, and what it does underneath.
fn row(ui: &mut Ui, name: &str, help: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).font(egui::FontId::new(13.0, theme::medium())));
        ui.add_space(8.0);
        control(ui);
    });
    ui.label(
        RichText::new(help)
            .color(theme::of(ui).text_muted)
            .size(12.0),
    );
    ui.add_space(6.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_choices_round_trip() {
        for r in [Reading::Automatic, Reading::Shown, Reading::Hidden] {
            assert_eq!(Reading::of(r.open()), r);
        }
    }
}
