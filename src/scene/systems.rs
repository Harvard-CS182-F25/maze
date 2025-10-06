use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use maze_generator::prelude::*;
use maze_generator::recursive_backtracking::RbGenerator;
use pyo3::prelude::*;

use crate::{
    agent::COLLISION_LAYER_AGENT,
    core::{MazeConfig, PyGridProvider},
    python::occupancy_grid::OccupancyGrid,
    scene::{
        COLLISION_LAYER_WALL, GridPlane, GridVisualization, WALL_HEIGHT, WALL_THICKNESS,
        WallBundle, WallGraphicsAssets,
    },
};

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

#[allow(clippy::too_many_arguments)]
fn push_horizontal(
    segs: &mut Vec<(Vec2, Vec2)>,
    x0: f32,
    z0: f32,
    cell: f32,
    row: i32,
    col: i32,
    pad: f32,
    xmin: f32,
    xmax: f32,
) {
    let z = z0 + (row as f32) * cell;
    let mut ax = x0 + (col as f32) * cell - pad;
    let mut bx = x0 + ((col + 1) as f32) * cell + pad;
    // keep within outer bounds
    ax = ax.max(xmin);
    bx = bx.min(xmax);
    segs.push((Vec2::new(ax, z), Vec2::new(bx, z)));
}

#[allow(clippy::too_many_arguments)]
fn push_vertical(
    segs: &mut Vec<(Vec2, Vec2)>,
    x0: f32,
    z0: f32,
    cell: f32,
    col: i32,
    row: i32,
    pad: f32,
    zmin: f32,
    zmax: f32,
) {
    let x = x0 + (col as f32) * cell;
    let mut az = z0 + (row as f32) * cell - pad;
    let mut bz = z0 + ((row + 1) as f32) * cell + pad;
    az = az.max(zmin);
    bz = bz.min(zmax);
    segs.push((Vec2::new(x, az), Vec2::new(x, bz)));
}

pub fn segments_from_maze(maze: &Maze, cell: f32, pad: f32) -> Vec<(Vec2, Vec2)> {
    let (w, h) = maze.size; // i32
    let x0 = -(w as f32) * cell * 0.5;
    let z0 = -(h as f32) * cell * 0.5;

    // outer bounds for clamping
    let xmin = x0;
    let xmax = x0 + (w as f32) * cell;
    let zmin = z0;
    let zmax = z0 + (h as f32) * cell;

    let mut segments = Vec::new();

    // top (north) border and left (west) border
    for c in 0..w {
        push_horizontal(&mut segments, x0, z0, cell, 0, c, pad, xmin, xmax);
    }
    for r in 0..h {
        push_vertical(&mut segments, x0, z0, cell, 0, r, pad, zmin, zmax);
    }

    // interior: add East/South walls where there is NO passage
    for y in 0..h {
        for x in 0..w {
            let field = maze.get_field(&Coordinates::new(x, y)).expect("in-bounds");
            if !field.has_passage(&Direction::East) {
                push_vertical(&mut segments, x0, z0, cell, x + 1, y, pad, zmin, zmax);
            }
            if !field.has_passage(&Direction::South) {
                push_horizontal(&mut segments, x0, z0, cell, y + 1, x, pad, xmin, xmax);
            }
        }
    }

    segments
}

pub fn spawn_walls(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    graphics: Option<Res<WallGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    let mut generator = RbGenerator::new(config.maze_generation.seed.map(|s| {
        let mut arr = [0u8; 32];
        arr[..4].copy_from_slice(&s.to_le_bytes());
        arr
    }));

    let maze = generator
        .generate(
            (config.maze_generation.width / config.maze_generation.cell_size).round() as i32,
            (config.maze_generation.height / config.maze_generation.cell_size).round() as i32,
        )
        .expect("Maze generation failed");

    let segments = segments_from_maze(
        &maze,
        config.maze_generation.cell_size,
        WALL_THICKNESS * 0.5,
    );

    for (p0, p1) in segments {
        let mut entity = commands.spawn(WallBundle::new(p0, p1, WALL_THICKNESS));

        if let (Some(meshes), Some(graphics)) = (&mut meshes, &graphics) {
            let len = p0.distance(p1);
            let mesh = meshes.add(Cuboid::new(len, WALL_HEIGHT, WALL_THICKNESS));
            entity.insert((Mesh3d(mesh), MeshMaterial3d(graphics.material.clone())));
        }
    }
}

pub fn spawn_grid_texture<T: PyGridProvider>(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<MazeConfig>,
    grid_res: Res<T>,
) {
    let (width, height, pixels) = Python::attach(|py| {
        let py_obj = grid_res.arc().read().unwrap().clone_ref(py);
        let grid_ref = py_obj.borrow(py);
        let w = grid_ref.width;
        let h = grid_ref.height;
        let pix = encode_grid_to_rgba(&grid_ref);
        (w, h, pix)
    });

    let mut img = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    img.sampler = ImageSampler::nearest();
    img.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;

    let handle = images.add(img);

    let world_w = config.maze_generation.width;
    let world_h = config.maze_generation.height;
    let mesh = meshes.add(Plane3d::default().mesh().size(world_w, world_h));
    let material = materials.add(StandardMaterial {
        base_color_texture: Some(handle.clone()),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material.clone()),
        Transform::from_xyz(0.0, WALL_HEIGHT, 0.0),
        GridPlane::<T>(std::marker::PhantomData),
        Name::new(format!("GridPlane<{}>", T::name())),
    ));

    commands.insert_resource(GridVisualization::<T> {
        handle,
        material,
        _marker: std::marker::PhantomData,
    });
}

pub fn update_grid_texture<T: PyGridProvider>(
    grid: Res<T>,
    mut vis: ResMut<GridVisualization<T>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (w, h, pixels) = Python::attach(|py| {
        let py_obj = grid.arc().read().unwrap().clone_ref(py);
        let grid_ref = py_obj.borrow(py);
        let w = grid_ref.width;
        let h = grid_ref.height;
        let pix = encode_grid_to_rgba(&grid_ref);
        (w, h, pix)
    });

    let material = materials
        .get_mut(&mut vis.material)
        .expect("get grid material");

    if let Some(img) = images.get_mut(&vis.handle)
        && img.size().x as usize == w
        && img.size().y as usize == h
    {
        if let Some(d) = img.data.as_mut() {
            d.copy_from_slice(&pixels)
        } else {
            img.data = Some(pixels);
        }
        material.base_color_texture = Some(vis.handle.clone());
    } else {
        warn!("Grid texture size changed, cannot update in place");
        let mut new_img = Image::new(
            Extent3d {
                width: w as u32,
                height: h as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        new_img.sampler = ImageSampler::nearest();
        new_img.texture_descriptor.usage = TextureUsages::COPY_DST
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT;
        images
            .insert(&mut vis.handle, new_img)
            .expect("re-insert grid texture");
        material.base_color_texture = Some(vis.handle.clone());
    }
}

fn encode_grid_to_rgba(grid: &OccupancyGrid) -> Vec<u8> {
    let width = grid.width;
    let height = grid.height;
    let mut buffer = vec![0u8; width * height * 4];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let entry = grid.grid[idx];

            let (r, g, b) = if entry.p_wall > 0.5 {
                (0u8, 0u8, 0u8)
            } else if entry.p_free > 0.5 {
                (255u8, 255u8, 255u8)
            } else if entry.p_flag > 0.5 {
                (255u8, 0u8, 0u8)
            } else if entry.p_capture_point > 0.5 {
                (0u8, 0u8, 255u8)
            } else {
                (127u8, 127u8, 127u8) // unknown
            };

            let off = idx * 4;
            buffer[off] = r;
            buffer[off + 1] = g;
            buffer[off + 2] = b;
            buffer[off + 3] = 200u8;
        }
    }

    buffer
}
