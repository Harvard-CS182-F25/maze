use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    agent::COLLISION_LAYER_AGENT,
    scene::{COLLISION_LAYER_WALL, WALL_HEIGHT, WALL_THICKNESS, WallBundle, WallGraphicsAssets},
};

pub fn spawn_walls(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    graphics: Option<Res<WallGraphicsAssets>>,
) {
    let outer = [
        (Vec2::new(-50.0, 50.0), Vec2::new(50.0, 50.0)),
        (Vec2::new(50.0, 50.0), Vec2::new(50.0, -50.0)),
        (Vec2::new(50.0, -50.0), Vec2::new(-50.0, -50.0)),
        (Vec2::new(-50.0, -50.0), Vec2::new(-50.0, 50.0)),
    ];

    let side_bars = [
        (Vec2::new(-45.0, 45.0), Vec2::new(-45.0, 5.0)),
        (Vec2::new(-45.0, -5.0), Vec2::new(-45.0, -45.0)),
        (Vec2::new(45.0, 45.0), Vec2::new(45.0, 5.0)),
        (Vec2::new(45.0, -5.0), Vec2::new(45.0, -45.0)),
    ];

    // Middle horizontal bars (purple in your plot)
    let middle = [
        (Vec2::new(-10.0, 5.0), Vec2::new(10.0, 5.0)),
        (Vec2::new(-10.0, -5.0), Vec2::new(10.0, -5.0)),
    ];

    // Center diamonds (from your Desmos polygons)
    let diamond_left_edges = [
        (Vec2::new(-5.0, 0.0), Vec2::new(-35.0, 30.0)),
        (Vec2::new(-35.0, -30.0), Vec2::new(-5.0, 0.0)),
        (Vec2::new(-5.0, 0.0), Vec2::new(25.0, -30.0)),
        (Vec2::new(25.0, 20.0), Vec2::new(5.0, 0.0)),
    ];

    let diamond_right_edges = [
        (Vec2::new(5.0, 0.0), Vec2::new(35.0, 30.0)),
        (Vec2::new(35.0, -30.0), Vec2::new(5.0, 0.0)),
        (Vec2::new(5.0, 0.0), Vec2::new(-25.0, 30.0)),
        (Vec2::new(-25.0, -20.0), Vec2::new(-5.0, 0.0)),
    ];

    let spawn_list = outer
        .into_iter()
        .chain(side_bars)
        .chain(middle)
        .chain(diamond_left_edges)
        .chain(diamond_right_edges);

    for (p0, p1) in spawn_list {
        let mut entity = commands.spawn(WallBundle::new(p0, p1, WALL_THICKNESS));

        if let (Some(meshes), Some(graphics)) = (&mut meshes, &graphics) {
            let len = p0.distance(p1);
            let mesh = meshes.add(Cuboid::new(len, WALL_HEIGHT, WALL_THICKNESS));
            entity.insert((Mesh3d(mesh), MeshMaterial3d(graphics.material.clone())));
        }
    }
}

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let mut entity = commands.spawn((
        Name::new("Ground Plane"),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(100.0, 1.0, 100.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
        CollisionLayers::new(
            LayerMask(COLLISION_LAYER_WALL),
            LayerMask(COLLISION_LAYER_AGENT),
        ),
    ));

    if let (Some(meshes), Some(materials)) = (&mut meshes, &mut materials) {
        let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let material = materials.add(Color::srgb(0.0, 1.0, 0.0));
        entity.insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}
