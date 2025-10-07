use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
};
use pyo3::prelude::*;

use crate::{
    core::MazeConfig,
    occupancy_grid::{GridPlane, GridVisualization, OccupancyGrid, PyGridProvider},
    scene::WALL_HEIGHT,
};

pub fn setup_key_instructions(mut commands: Commands) {
    commands.spawn((
        Text::new("O: Toggle Occupancy Grid | T: Toggle True Grid"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(5.0),
            right: Val::Px(5.0),
            ..default()
        },
    ));
}

pub fn toggle_grid<T: PyGridProvider>(mut query: Query<&mut Visibility, With<GridPlane<T>>>) {
    for mut vis in query.iter_mut() {
        vis.toggle_inherited_hidden();
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
        Visibility::Hidden,
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

            let (r, g, b, a) = if entry.p_wall > 0.5 {
                (0u8, 0u8, 0u8, 200u8)
            } else if entry.p_free > 0.5 {
                (255u8, 255u8, 255u8, 0u8)
            } else if entry.p_flag > 0.5 {
                (255u8, 0u8, 0u8, 200u8)
            } else if entry.p_capture_point > 0.5 {
                (0u8, 0u8, 255u8, 200u8)
            } else {
                (127u8, 127u8, 127u8, 200u8) // unknown
            };

            let off = idx * 4;
            buffer[off] = r;
            buffer[off + 1] = g;
            buffer[off + 2] = b;
            buffer[off + 3] = a;
        }
    }

    buffer
}
