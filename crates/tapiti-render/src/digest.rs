//! A fingerprint of what a floor looks like in 3D: the hash of its mesh and of two pictures of it (from outside and from the entrance).
//! The golden test of this crate and the native-vs-WebAssembly comparison both use it, so a change in how a floor is built or drawn
//! is seen as a change of the fingerprint, and a machine or a build that draws it differently is too.

use crate::mesh::{build, Mesh, MeshOptions};
use crate::raster::{render, Camera, Frame};
use crate::view::Channel;
use crate::walk::{WalkMap, EYE};
use tapiti_core::generate::Floor;

fn fnv(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The hash of every number of a mesh.
pub fn mesh_hash(m: &Mesh) -> u64 {
    fnv(m
        .positions
        .iter()
        .chain(&m.normals)
        .flatten()
        .flat_map(|v| v.to_bits().to_le_bytes())
        .chain(m.colors.iter().flatten().copied())
        .chain(m.indices.iter().flat_map(|i| i.to_le_bytes()))
        .chain(m.owner.iter().flat_map(|i| i.to_le_bytes())))
}

pub fn frame_hash(f: &Frame) -> u64 {
    fnv(f.rgba.iter().copied())
}

/// The mesh hash (solid, then water), and the hashes of the picture from outside and of the one from the entrance.
pub fn of(floor: &Floor) -> [u64; 3] {
    let options = MeshOptions {
        tint: [Channel::Temperature, Channel::Height]
            .into_iter()
            .find(|c| c.available(floor)),
        light: Channel::Light.available(floor),
        ..MeshOptions::default()
    };
    let scene = build(floor, &options);
    let mesh = fnv(mesh_hash(&scene.solid)
        .to_le_bytes()
        .into_iter()
        .chain(mesh_hash(&scene.water).to_le_bytes()));
    let (lo, hi) = (scene.lo, scene.hi);
    let target = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let reach = (hi[0] - lo[0]).max(hi[2] - lo[2]).max(100.0);
    let outside = render(
        &scene,
        &Camera::orbit(target, 0.6, -0.75, reach * 0.9),
        160,
        120,
        [200, 196, 180],
    );
    // From the entrance, with roofs over the rooms, as the walk draws it.
    let roofed = MeshOptions {
        ceilings: true,
        ..options
    };
    let inside_scene = build(floor, &roofed);
    let inside = match WalkMap::new(floor, &roofed).start() {
        Some((feet, yaw)) => {
            let camera = Camera {
                eye: [feet[0], feet[1] + EYE, feet[2]],
                yaw,
                pitch: 0.0,
                fov: 70.0,
                near: 2.0,
                far: 3000.0,
            };
            frame_hash(&render(&inside_scene, &camera, 160, 120, [200, 196, 180]))
        }
        None => 0,
    };
    [mesh, frame_hash(&outside), inside]
}
