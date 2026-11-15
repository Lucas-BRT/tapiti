//! A small software rasteriser for the meshes of [`crate::mesh`]: perspective, a depth buffer, flat shading with the sun and a touch of
//! fog, the water blended over. It is the 3D view of the desktop: no GPU, no `unsafe`, the same pixels on every machine, and a frame
//! can be checked in a test.

use crate::mesh::{Mesh, Scene};

/// Where the scene is looked at from: an eye, the way it faces and the lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: [f32; 3],
    /// Turn about the vertical, in radians: 0 looks along +z (the plan's `y`), growing toward +x.
    pub yaw: f32,
    /// Up (positive) or down, in radians.
    pub pitch: f32,
    /// The vertical field of view, in degrees.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    /// A camera that circles `target` at `distance`, looking at it.
    pub fn orbit(target: [f32; 3], yaw: f32, pitch: f32, distance: f32) -> Self {
        let f = Self::forward_of(yaw, pitch);
        Self {
            eye: [
                target[0] - f[0] * distance,
                target[1] - f[1] * distance,
                target[2] - f[2] * distance,
            ],
            yaw,
            pitch,
            fov: 50.0,
            near: (distance * 0.02).max(0.5),
            far: distance * 6.0 + 2000.0,
        }
    }

    fn forward_of(yaw: f32, pitch: f32) -> [f32; 3] {
        // `libm` on every target: the platform's sin and cos differ in the last digit, and a frame must be the same everywhere.
        [
            libm::cosf(pitch) * libm::sinf(yaw),
            libm::sinf(pitch),
            libm::cosf(pitch) * libm::cosf(yaw),
        ]
    }

    pub fn forward(&self) -> [f32; 3] {
        Self::forward_of(self.yaw, self.pitch)
    }
}

/// A finished picture, 8 bits a channel, top row first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = dot(v, v).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// A vertex in camera space: right, up, forward.
type Cam = [f32; 3];

struct View {
    eye: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
    fx: f32,
    fy: f32,
    near: f32,
    far: f32,
    w: f32,
    h: f32,
}

impl View {
    fn new(c: &Camera, w: usize, h: usize) -> Self {
        let forward = c.forward();
        let right = norm(cross(forward, [0.0, 1.0, 0.0]));
        let up = cross(right, forward);
        let fy = 1.0 / libm::tanf(c.fov.to_radians() / 2.0);
        Self {
            eye: c.eye,
            right,
            up,
            forward,
            fx: fy * h as f32 / w as f32,
            fy,
            near: c.near,
            far: c.far,
            w: w as f32,
            h: h as f32,
        }
    }

    fn cam(&self, p: [f32; 3]) -> Cam {
        let d = [p[0] - self.eye[0], p[1] - self.eye[1], p[2] - self.eye[2]];
        [dot(d, self.right), dot(d, self.up), dot(d, self.forward)]
    }

    /// Screen x, screen y (down) and 1/z of a point in front of the near plane.
    fn screen(&self, c: Cam) -> [f32; 3] {
        [
            (c[0] * self.fx / c[2] * 0.5 + 0.5) * self.w,
            (0.5 - c[1] * self.fy / c[2] * 0.5) * self.h,
            1.0 / c[2],
        ]
    }
}

/// A triangle against the near plane: the triangles left of it (0, 1 or 2).
fn clip_near(t: [Cam; 3], near: f32) -> Vec<[Cam; 3]> {
    let inside: Vec<bool> = t.iter().map(|v| v[2] >= near).collect();
    let cut = |a: Cam, b: Cam| {
        let k = (near - a[2]) / (b[2] - a[2]);
        [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, near]
    };
    match inside.iter().filter(|i| **i).count() {
        3 => vec![t],
        0 => Vec::new(),
        1 => {
            let i = inside.iter().position(|i| *i).unwrap();
            let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
            vec![[a, cut(a, b), cut(a, c)]]
        }
        _ => {
            let o = inside.iter().position(|i| !*i).unwrap();
            let (a, b, c) = (t[o], t[(o + 1) % 3], t[(o + 2) % 3]);
            let (ab, ac) = (cut(b, a), cut(c, a));
            vec![[b, c, ac], [b, ac, ab]]
        }
    }
}

struct Target<'a> {
    w: usize,
    h: usize,
    rgb: &'a mut [f32],
    depth: &'a mut [f32],
}

/// Fills a triangle: `blend` of 1 draws it solid and writes the depth; less blends it over what is there and leaves the depth.
fn fill(t: &mut Target, s: [[f32; 3]; 3], color: [f32; 3], blend: f32) {
    let min_x = s
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i64;
    let max_x = s
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(t.w as f32 - 1.0) as i64;
    let min_y = s
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i64;
    let max_y = s
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(t.h as f32 - 1.0) as i64;
    let area =
        (s[1][0] - s[0][0]) * (s[2][1] - s[0][1]) - (s[1][1] - s[0][1]) * (s[2][0] - s[0][0]);
    if area.abs() < 1e-6 || min_x > max_x || min_y > max_y {
        return;
    }
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let e0 = ((s[2][0] - s[1][0]) * (p[1] - s[1][1])
                - (s[2][1] - s[1][1]) * (p[0] - s[1][0]))
                / area;
            let e1 = ((s[0][0] - s[2][0]) * (p[1] - s[2][1])
                - (s[0][1] - s[2][1]) * (p[0] - s[2][0]))
                / area;
            let e2 = 1.0 - e0 - e1;
            if e0 < 0.0 || e1 < 0.0 || e2 < 0.0 {
                continue;
            }
            let inv_z = e0 * s[0][2] + e1 * s[1][2] + e2 * s[2][2];
            let i = y as usize * t.w + x as usize;
            if inv_z <= t.depth[i] {
                continue;
            }
            if blend >= 1.0 {
                t.depth[i] = inv_z;
                t.rgb[i * 3..i * 3 + 3].copy_from_slice(&color);
            } else {
                for (dst, src) in t.rgb[i * 3..i * 3 + 3].iter_mut().zip(color) {
                    *dst += (src - *dst) * blend;
                }
            }
        }
    }
}

fn draw(t: &mut Target, view: &View, mesh: &Mesh, sun: [f32; 3], fog: [f32; 3], blend: f32) {
    for tri in mesh.indices.as_chunks::<3>().0 {
        let i = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let n = mesh.normals[i[0]];
        let c = mesh.colors[i[0]];
        let cam = [
            view.cam(mesh.positions[i[0]]),
            view.cam(mesh.positions[i[1]]),
            view.cam(mesh.positions[i[2]]),
        ];
        // Lit from the sun on both sides, the way a thin wall is.
        let light = 0.42 + 0.58 * dot(n, sun).abs();
        for part in clip_near(cam, view.near) {
            let depth = (part[0][2] + part[1][2] + part[2][2]) / 3.0;
            let mist = (depth / view.far).clamp(0.0, 1.0) * 0.5;
            let base = [0, 1, 2].map(|k| f32::from(c[k]) / 255.0 * light);
            let color = [0, 1, 2].map(|k| base[k] + (fog[k] - base[k]) * mist);
            let s = [
                view.screen(part[0]),
                view.screen(part[1]),
                view.screen(part[2]),
            ];
            fill(t, s, color, blend);
        }
    }
}

/// Draws the scene: the solid mesh, then the water over it. `background` is the sky and the fog.
pub fn render(
    scene: &Scene,
    camera: &Camera,
    width: usize,
    height: usize,
    background: [u8; 3],
) -> Frame {
    let (width, height) = (width.max(1), height.max(1));
    let view = View::new(camera, width, height);
    let fog = background.map(|c| f32::from(c) / 255.0);
    let mut rgb: Vec<f32> = (0..width * height).flat_map(|_| fog).collect();
    let mut depth = vec![0.0_f32; width * height];
    let sun = norm([0.4, 0.8, 0.3]);
    {
        let mut target = Target {
            w: width,
            h: height,
            rgb: &mut rgb,
            depth: &mut depth,
        };
        draw(&mut target, &view, &scene.solid, sun, fog, 1.0);
        draw(&mut target, &view, &scene.water, sun, fog, 0.45);
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for px in rgb.as_chunks::<3>().0 {
        rgba.extend(px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        rgba.push(255);
    }
    Frame {
        width,
        height,
        rgba,
    }
}

/// The room under the pixel `(x, y)` of a picture `width` by `height` drawn from `camera`: the nearest triangle of the solid mesh the
/// ray through it hits, if it belongs to a room (a corridor or a stair gives `None`).
pub fn pick(
    scene: &Scene,
    camera: &Camera,
    width: usize,
    height: usize,
    x: f32,
    y: f32,
) -> Option<usize> {
    let view = View::new(camera, width.max(1), height.max(1));
    let (nx, ny) = ((x / view.w - 0.5) * 2.0, (0.5 - y / view.h) * 2.0);
    let local = [nx / view.fx, ny / view.fy, 1.0];
    let dir = norm([
        view.right[0] * local[0] + view.up[0] * local[1] + view.forward[0] * local[2],
        view.right[1] * local[0] + view.up[1] * local[1] + view.forward[1] * local[2],
        view.right[2] * local[0] + view.up[2] * local[1] + view.forward[2] * local[2],
    ]);
    let m = &scene.solid;
    let mut best: Option<(f32, u32)> = None;
    for (k, tri) in m.indices.as_chunks::<3>().0.iter().enumerate() {
        let [a, b, c] = [0, 1, 2].map(|i| m.positions[tri[i] as usize]);
        // Möller-Trumbore, on both sides of the triangle.
        let (e1, e2) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let p = cross(dir, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-9 {
            continue;
        }
        let t0 = [view.eye[0] - a[0], view.eye[1] - a[1], view.eye[2] - a[2]];
        let u = dot(t0, p) / det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(t0, e1);
        let v = dot(dir, q) / det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = dot(e2, q) / det;
        if t > view.near && best.is_none_or(|(d, _)| t < d) {
            best = Some((t, m.owner.get(k).copied().unwrap_or(crate::mesh::NO_ROOM)));
        }
    }
    best.and_then(|(_, o)| (o != crate::mesh::NO_ROOM).then_some(o as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(y: f32, z: f32, size: f32, color: [u8; 3]) -> Mesh {
        let mut m = Mesh::default();
        for p in [
            [-size, y - size, z],
            [size, y - size, z],
            [size, y + size, z],
            [-size, y + size, z],
        ] {
            m.positions.push(p);
            m.normals.push([0.0, 0.0, 1.0]);
            m.colors.push(color);
        }
        m.indices = vec![0, 1, 2, 0, 2, 3];
        m
    }

    fn scene(solid: Mesh, water: Mesh) -> Scene {
        Scene {
            solid,
            water,
            lo: [0.0; 3],
            hi: [0.0; 3],
        }
    }

    fn pixel(f: &Frame, x: usize, y: usize) -> [u8; 3] {
        let i = (y * f.width + x) * 4;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    }

    #[test]
    fn a_quad_in_front_covers_the_middle_and_leaves_the_corners_to_the_sky() {
        let cam = Camera {
            eye: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 60.0,
            near: 1.0,
            far: 1000.0,
        };
        let mut m = quad(0.0, 50.0, 20.0, [200, 40, 40]);
        m.colors.iter_mut().for_each(|c| *c = [200, 40, 40]);
        let f = render(&scene(m, Mesh::default()), &cam, 64, 64, [10, 10, 10]);
        assert_ne!(pixel(&f, 32, 32), [10, 10, 10]);
        assert_eq!(pixel(&f, 1, 1), [10, 10, 10]);
        assert!(
            pixel(&f, 32, 32)[0] > pixel(&f, 32, 32)[1],
            "the quad is red"
        );
    }

    #[test]
    fn the_nearer_quad_hides_the_farther_one_whatever_the_order() {
        let cam = Camera {
            eye: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 60.0,
            near: 1.0,
            far: 1000.0,
        };
        let (near, far) = (
            quad(0.0, 30.0, 20.0, [0, 200, 0]),
            quad(0.0, 80.0, 60.0, [0, 0, 200]),
        );
        let mut both = near.clone();
        let base = both.positions.len() as u32;
        both.positions.extend(&far.positions);
        both.normals.extend(&far.normals);
        both.colors.extend(&far.colors);
        both.indices.extend(far.indices.iter().map(|i| i + base));
        let a = render(&scene(both, Mesh::default()), &cam, 64, 64, [0, 0, 0]);
        let mut reversed = far.clone();
        let base = reversed.positions.len() as u32;
        reversed.positions.extend(&near.positions);
        reversed.normals.extend(&near.normals);
        reversed.colors.extend(&near.colors);
        reversed
            .indices
            .extend(near.indices.iter().map(|i| i + base));
        let b = render(&scene(reversed, Mesh::default()), &cam, 64, 64, [0, 0, 0]);
        assert_eq!(a, b, "the order of the triangles changed the picture");
        assert!(
            pixel(&a, 32, 32)[1] > pixel(&a, 32, 32)[2],
            "the green quad is in front"
        );
    }

    #[test]
    fn a_triangle_through_the_eye_is_clipped_and_nothing_behind_is_drawn() {
        let cam = Camera {
            eye: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 60.0,
            near: 1.0,
            far: 1000.0,
        };
        let mut m = Mesh::default();
        for p in [
            [-30.0, -30.0, -20.0],
            [30.0, -30.0, 40.0],
            [0.0, 30.0, 40.0],
        ] {
            m.positions.push(p);
            m.normals.push([0.0, 0.0, 1.0]);
            m.colors.push([255, 255, 255]);
        }
        m.indices = vec![0, 1, 2];
        let f = render(&scene(m, Mesh::default()), &cam, 48, 48, [0, 0, 0]);
        assert!(
            f.rgba.as_chunks::<4>().0.iter().any(|p| p[0] > 0),
            "the part in front was not drawn"
        );
        let behind = quad(0.0, -50.0, 20.0, [255, 255, 255]);
        let f = render(&scene(behind, Mesh::default()), &cam, 48, 48, [0, 0, 0]);
        assert!(
            f.rgba.as_chunks::<4>().0.iter().all(|p| p[0] == 0),
            "something behind the eye was drawn"
        );
    }

    #[test]
    fn water_is_blended_over_what_is_under_it_and_a_frame_repeats() {
        let cam = Camera {
            eye: [0.0, 0.0, 0.0],
            yaw: 0.0,
            pitch: 0.0,
            fov: 60.0,
            near: 1.0,
            far: 1000.0,
        };
        let solid = quad(0.0, 60.0, 40.0, [200, 200, 200]);
        let water = quad(0.0, 40.0, 40.0, [20, 120, 200]);
        let s = scene(solid.clone(), water);
        let a = render(&s, &cam, 48, 48, [0, 0, 0]);
        let dry = render(&scene(solid, Mesh::default()), &cam, 48, 48, [0, 0, 0]);
        assert_ne!(pixel(&a, 24, 24), pixel(&dry, 24, 24));
        assert!(
            pixel(&a, 24, 24)[2] > pixel(&a, 24, 24)[0],
            "bluer under water"
        );
        assert_eq!(a, render(&s, &cam, 48, 48, [0, 0, 0]));
    }

    #[test]
    fn an_orbit_camera_looks_at_its_target() {
        let c = Camera::orbit([10.0, 5.0, -3.0], 0.7, 0.3, 100.0);
        let to_target = norm([10.0 - c.eye[0], 5.0 - c.eye[1], -3.0 - c.eye[2]]);
        let f = c.forward();
        assert!((0..3).all(|k| (to_target[k] - f[k]).abs() < 1e-5));
    }
}
