//! The 3D view: the floor as a mesh drawn by the software rasteriser of `tapiti-render`, from an orbiting camera or on foot inside it.
//! The picture is a texture the size of the map: no GPU, so it looks the same everywhere.

use crate::map::Palette;
use crate::Overlays;
use eframe::egui::{
    self, Color32, ColorImage, Key, Pos2, Rect, Response, TextureHandle, TextureOptions, Ui,
};
use std::sync::Arc;
use tapiti_core::generate::Floor;
use tapiti_render::mesh::{build, MeshOptions, Scene};
use tapiti_render::raster::{render, Camera};
use tapiti_render::view::Channel;
use tapiti_render::walk::{WalkMap, EYE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Circling the floor from outside.
    Orbit,
    /// Standing in it.
    Walk,
}

/// What the 3D view keeps between frames.
pub struct Solid {
    pub mode: Mode,
    pub yaw: f32,
    pub pitch: f32,
    /// The orbit distance as a multiple of the size of the floor.
    pub zoom: f32,
    pub wall_height: f64,
    pub exaggeration: f64,
    pub explode: f64,
    pub up_to: Option<usize>,
    /// Pixels drawn per point of the map: below 1 is quicker and softer.
    pub quality: f32,
    /// The feet of the walker.
    feet: [f32; 3],
    walk_yaw: f32,
    walk_pitch: f32,
    built: Option<Built>,
    texture: Option<TextureHandle>,
    /// What the last picture was drawn from, so an idle view is not drawn again.
    drawn: Option<(Vec<u32>, [usize; 2])>,
}

struct Built {
    key: Vec<u32>,
    scene: Scene,
    walk: WalkMap,
}

impl Default for Solid {
    fn default() -> Self {
        Self {
            mode: Mode::Orbit,
            yaw: 0.6,
            pitch: 0.75,
            zoom: 0.9,
            wall_height: 60.0,
            exaggeration: 2.0,
            explode: 0.5,
            up_to: None,
            quality: 0.8,
            feet: [0.0; 3],
            walk_yaw: 0.0,
            walk_pitch: 0.0,
            built: None,
            texture: None,
            drawn: None,
        }
    }
}

impl Solid {
    fn options(&self, overlays: Overlays) -> MeshOptions {
        MeshOptions {
            wall_height: self.wall_height,
            exaggeration: self.exaggeration,
            explode: self.explode,
            up_to: self.up_to,
            ceilings: self.mode == Mode::Walk,
            tint: if overlays.temperature {
                Some(Channel::Temperature)
            } else if overlays.height {
                Some(Channel::Height)
            } else {
                None
            },
            light: overlays.light,
            water: overlays.water,
        }
    }

    /// What the mesh depends on, as a list of numbers that change when it has to be built again.
    fn key(&self, floor: &Arc<Floor>, o: &MeshOptions) -> Vec<u32> {
        vec![
            Arc::as_ptr(floor) as usize as u32,
            (Arc::as_ptr(floor) as usize >> 32) as u32,
            (o.wall_height as f32).to_bits(),
            (o.exaggeration as f32).to_bits(),
            (o.explode as f32).to_bits(),
            o.up_to.map_or(u32::MAX, |u| u as u32),
            u32::from(o.ceilings),
            o.tint.map_or(9, |c| c as u32),
            u32::from(o.light),
            u32::from(o.water),
            floor.environment.as_ref().map_or(0, |e| {
                // The layers can be reevaluated in place: their settings are part of what the mesh shows.
                (e.height.is_some() as u32)
                    | ((e.water.is_some() as u32) << 1)
                    | ((e.light.is_some() as u32) << 2)
                    | ((e.climate.is_some() as u32) << 3)
            }),
            floor
                .environment
                .as_ref()
                .and_then(|e| e.water.as_ref())
                .map_or(0, |w| w.volume.to_bits() as u32),
            floor
                .environment
                .as_ref()
                .and_then(|e| e.light.as_ref())
                .map_or(0, |l| l.intensity.iter().sum::<f64>().to_bits() as u32),
        ]
    }

    fn ensure(&mut self, floor: &Arc<Floor>, overlays: Overlays) {
        let o = self.options(overlays);
        let key = self.key(floor, &o);
        if self.built.as_ref().is_some_and(|b| b.key == key) {
            return;
        }
        let walk = WalkMap::new(floor, &o);
        let was_walking = self.built.is_some();
        self.built = Some(Built {
            key,
            scene: build(floor, &o),
            walk,
        });
        if !was_walking || self.mode == Mode::Walk {
            self.respawn();
        }
        self.drawn = None;
    }

    /// Back to the entrance, on foot.
    pub fn respawn(&mut self) {
        if let Some((feet, yaw)) = self.built.as_ref().and_then(|b| b.walk.start()) {
            self.feet = feet;
            self.walk_yaw = yaw;
            self.walk_pitch = 0.0;
        }
    }

    pub fn reset_orbit(&mut self) {
        let d = Self::default();
        (self.yaw, self.pitch, self.zoom) = (d.yaw, d.pitch, d.zoom);
    }

    /// Draws the floor in the area `rect` and handles the mouse and the keys over it.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        response: &Response,
        floor: &Arc<Floor>,
        overlays: Overlays,
        pal: &Palette,
    ) {
        self.ensure(floor, overlays);
        let ctx = ui.ctx().clone();
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let delta = response.drag_delta();
        let mut moved = false;
        match self.mode {
            Mode::Orbit => {
                if response.dragged() {
                    self.yaw -= delta.x * 0.01;
                    self.pitch = (self.pitch + delta.y * 0.008).clamp(0.05, 1.5);
                    moved = true;
                }
                if response.hovered() {
                    let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0.0 {
                        self.zoom = (self.zoom * (-scroll * 0.0015).exp()).clamp(0.08, 4.0);
                        moved = true;
                    }
                }
            }
            Mode::Walk => {
                if response.dragged() {
                    self.walk_yaw -= delta.x * 0.006;
                    self.walk_pitch = (self.walk_pitch - delta.y * 0.006).clamp(-1.3, 1.3);
                    moved = true;
                }
                let (axis, run, stair) = ctx.input(|i| {
                    let key = |k: Key| f32::from(i.key_down(k));
                    (
                        [
                            key(Key::D) + key(Key::ArrowRight) - key(Key::A) - key(Key::ArrowLeft),
                            key(Key::W) + key(Key::ArrowUp) - key(Key::S) - key(Key::ArrowDown),
                        ],
                        i.modifiers.shift,
                        i.key_pressed(Key::E),
                    )
                });
                if let Some(b) = self.built.as_ref() {
                    if axis != [0.0, 0.0] {
                        let speed = dt * 90.0 * if run { 2.2 } else { 1.0 };
                        let (s, c) = self.walk_yaw.sin_cos();
                        let (dx, dz) = (s * axis[1] + c * axis[0], c * axis[1] - s * axis[0]);
                        b.walk.step(&mut self.feet, dx * speed, dz * speed, 6.0);
                        moved = true;
                    }
                    if stair {
                        if let Some(y) = b.walk.stair_at(self.feet, 22.0) {
                            self.feet[1] = y;
                            moved = true;
                        }
                    }
                }
                if axis != [0.0, 0.0] {
                    ctx.request_repaint();
                }
            }
        }
        let Some(b) = self.built.as_ref() else { return };
        let size = [
            ((rect.width() * self.quality) as usize).clamp(64, 2400),
            ((rect.height() * self.quality) as usize).clamp(64, 1600),
        ];
        let camera = match self.mode {
            Mode::Orbit => {
                let (lo, hi) = (b.scene.lo, b.scene.hi);
                let target = [
                    (lo[0] + hi[0]) / 2.0,
                    (lo[1] + hi[1]) / 2.0,
                    (lo[2] + hi[2]) / 2.0,
                ];
                let diag = ((hi[0] - lo[0]).powi(2) + (hi[2] - lo[2]).powi(2))
                    .sqrt()
                    .max(100.0);
                // Looking from the south, so the plan reads the way it does on the map.
                Camera::orbit(target, self.yaw, -self.pitch, diag * self.zoom)
            }
            Mode::Walk => Camera {
                eye: [self.feet[0], self.feet[1] + EYE, self.feet[2]],
                yaw: self.walk_yaw,
                pitch: self.walk_pitch,
                fov: 70.0,
                near: 2.0,
                far: 3000.0,
            },
        };
        let stamp: Vec<u32> = [
            camera.eye[0],
            camera.eye[1],
            camera.eye[2],
            camera.yaw,
            camera.pitch,
            camera.fov,
        ]
        .iter()
        .map(|v| v.to_bits())
        .collect();
        let fresh = self
            .drawn
            .as_ref()
            .is_some_and(|(s, z)| *s == stamp && *z == size)
            && self.texture.is_some();
        if !fresh || moved {
            let sky = if pal.dark {
                [0x1c, 0x1b, 0x20]
            } else {
                [0xea, 0xe4, 0xd2]
            };
            let frame = render(&b.scene, &camera, size[0], size[1], sky);
            let image =
                ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &frame.rgba);
            match self.texture.as_mut() {
                Some(t) => t.set(image, TextureOptions::LINEAR),
                None => {
                    self.texture = Some(ctx.load_texture("view3d", image, TextureOptions::LINEAR))
                }
            }
            self.drawn = Some((stamp, size));
        }
        if let Some(t) = &self.texture {
            ui.painter_at(rect).image(
                t.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if self.mode == Mode::Walk {
            let c = rect.center();
            let ink = Color32::from_white_alpha(170);
            let p = ui.painter_at(rect);
            p.line_segment(
                [c - egui::vec2(6.0, 0.0), c + egui::vec2(6.0, 0.0)],
                egui::Stroke::new(1.5, ink),
            );
            p.line_segment(
                [c - egui::vec2(0.0, 6.0), c + egui::vec2(0.0, 6.0)],
                egui::Stroke::new(1.5, ink),
            );
        }
    }
}
