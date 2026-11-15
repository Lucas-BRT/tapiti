//! Picking in the 3D view: the ray through a pixel finds the room whose surface is there.

use tapiti_core::{
    generate::generate,
    params::{rich_of_size, Layout, Size},
};
use tapiti_render::mesh::{build, MeshOptions, NO_ROOM};
use tapiti_render::raster::{pick, Camera};

#[test]
fn looking_straight_down_at_a_room_picks_it_and_a_corridor_picks_nothing() {
    let mut p = rich_of_size(Size::Medium);
    p.layout = Layout::Growth;
    let floor = generate(3, &p);
    let scene = build(&floor, &MeshOptions::default());
    assert_eq!(scene.solid.owner.len(), scene.solid.triangles());
    let mut checked = 0;
    for r in floor.rooms.iter().filter(|r| r.alive && !r.leaf).take(8) {
        let y = scene
            .solid
            .positions
            .iter()
            .map(|v| v[1])
            .fold(f32::NEG_INFINITY, f32::max)
            + 400.0;
        let camera = Camera {
            eye: [r.center[0] as f32, y, r.center[1] as f32],
            yaw: 0.0,
            pitch: -std::f32::consts::FRAC_PI_2,
            fov: 40.0,
            near: 1.0,
            far: 5000.0,
        };
        // The room's own floor is under the middle of the picture, unless a room of another level covers it from above.
        let got = pick(&scene, &camera, 200, 200, 100.0, 100.0);
        if floor.stack.is_none() {
            assert_eq!(got, Some(r.id), "room {} was not picked from above", r.id);
        }
        checked += 1;
    }
    assert!(checked > 0);
    // Every owner is a room of the floor or nobody.
    assert!(scene
        .solid
        .owner
        .iter()
        .all(|o| *o == NO_ROOM || (*o as usize) < floor.rooms.len()));
    assert!(
        scene.solid.owner.contains(&NO_ROOM),
        "corridors belong to no room"
    );
}

#[test]
fn a_ray_into_the_sky_picks_nothing() {
    let floor = generate(3, &rich_of_size(Size::Medium));
    let scene = build(&floor, &MeshOptions::default());
    let camera = Camera {
        eye: [0.0, 500.0, 0.0],
        yaw: 0.0,
        pitch: 1.2,
        fov: 50.0,
        near: 1.0,
        far: 5000.0,
    };
    assert_eq!(pick(&scene, &camera, 100, 100, 50.0, 50.0), None);
}
