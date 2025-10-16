use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
    window::PrimaryWindow,
};
use pyo3::prelude::*;

use crate::{
    core::MazeConfig,
    occupancy_grid::{
        GridPlane, GridVisualization, HoverBox, HoverBoxText, HoverCell, OccupancyGrid,
        PyGridProvider,
    },
    python::game_state::EntityType,
    scene::WALL_HEIGHT,
};

pub fn setup_key_instructions(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::Grid,
                top: Val::Px(5.0),
                right: Val::Px(5.0),
                padding: Val::Px(2.5).into(),
                justify_items: JustifyItems::End,
                align_items: AlignItems::Start,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("O: Toggle Computed Occupancy Grid | T: Toggle True Occupancy Grid"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
            ));
            parent.spawn((
                Text::new("+/-: Zoom In/Out | Arrow Keys: Pan Camera"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
            ));
        });
}

#[allow(clippy::type_complexity)]
pub fn toggle_grid<TOn: PyGridProvider, TOff: PyGridProvider>(
    mut vis_on: Query<&mut Visibility, (With<GridPlane<TOn>>, Without<GridPlane<TOff>>)>,
    mut vis_off: Query<&mut Visibility, (With<GridPlane<TOff>>, Without<GridPlane<TOn>>)>,
) {
    for mut vis in vis_on.iter_mut() {
        vis.toggle_inherited_hidden();

        if *vis != Visibility::Hidden {
            for mut vis2 in vis_off.iter_mut() {
                *vis2 = Visibility::Hidden;
            }
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

            let (r, g, b, a) = match entry.assignment {
                Some(EntityType::Wall) => (0u8, 0u8, 0u8, 200u8),
                Some(EntityType::Empty) => (0u8, 0u8, 0u8, 0u8),
                Some(EntityType::Flag) => (219u8, 112u8, 147u8, 200u8),
                Some(EntityType::CapturePoint) => (199u8, 21u8, 133u8, 200u8),
                _ => (127u8, 127u8, 127u8, 100u8),
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

pub fn cursor_to_grid_cell<T: PyGridProvider>(
    // cursor
    windows: Query<&Window, With<PrimaryWindow>>,
    // camera doing the looking
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    // your grid plane transform
    plane_q: Query<&GlobalTransform, With<GridPlane<T>>>,
    // your sizes
    config: Res<MazeConfig>,
    mut hover: ResMut<HoverCell>,
) {
    // let window = if let Ok(w) = windows.single() {
    //     w
    // } else {
    //     return;
    // };
    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_transform)) = cams.single() else {
        return;
    };
    let Ok(plane_gt) = plane_q.single() else {
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        *hover = HoverCell::default();
        return;
    };

    // Build a world-space ray from the cursor
    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor) else {
        return;
    };

    let ro = ray.origin;
    let rd = ray.direction;

    // Intersect ray with the plane the grid image sits on.
    // This assumes the plane is an XZ "floor", so its normal is +Y in local space.
    let plane_point = plane_gt.translation();
    let plane_normal = plane_gt.up().into(); // world-space normal (+Y rotated by plane)

    let denom = rd.dot(plane_normal);
    if denom.abs() < 1e-6 {
        *hover = HoverCell::default();
        return; // ray is parallel to plane
    }
    let t = (plane_point - ro).dot(plane_normal) / denom;
    if t < 0.0 {
        *hover = HoverCell::default();
        return; // plane behind camera
    }
    let hit = ro + t * rd;

    // Convert world hit → plane-local so we can use X/Z cleanly
    let inv = plane_gt.to_matrix().inverse();
    let local = inv.transform_point3(hit); // local.y should be ~0

    // Map local.x/local.z to [0, width)×[0, height)
    let world_w = config.maze_generation.width;
    let world_h = config.maze_generation.height;
    let cell = config.agent.occupancy_grid_cell_size;
    let grid_w = (world_w / cell).round() as u32;
    let grid_h = (world_h / cell).round() as u32;

    // Plane is centered at (0, WALL_HEIGHT, 0) with extents ±world_w/2, ±world_h/2
    let u = (local.x + world_w * 0.5) / cell; // column (x)
    let v = (local.z + world_h * 0.5) / cell; // row    (z)

    let col = u.floor() as i32;
    let row = v.floor() as i32;

    // Inside?
    if col < 0 || row < 0 || col as u32 >= grid_w || row as u32 >= grid_h {
        *hover = HoverCell::default();
        return;
    }

    // If your image ends up vertically flipped, swap to: let row = (grid_h as i32 - 1) - row;
    hover.cell = Some(UVec2::new(col as u32, row as u32));
    hover.world_hit = Some(hit);
}

pub fn setup_hover_box<T: 'static + Resource>(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BorderRadius::all(Val::Px(4.0)),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            GlobalZIndex(10),
            HoverBox {
                _marker: std::marker::PhantomData::<T>,
            },
            Name::new("HoverBox"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Left),
                HoverBoxText,
            ));
        });
}

pub fn update_hover_box<T: PyGridProvider>(
    windows: Query<&Window, With<PrimaryWindow>>,
    plane_vis: Query<&Visibility, With<GridPlane<T>>>,
    grid: Res<T>,
    mut q_box: Query<(Entity, &mut Node, &mut BackgroundColor), With<HoverBox<T>>>,
    mut q_text: Query<(&mut Text, &ChildOf), With<HoverBoxText>>,
    hover: Res<HoverCell>,
) {
    let Ok(window) = windows.single() else {
        info!("No window for tooltip");
        return;
    };
    let Ok(vis) = plane_vis.single() else {
        info!("No plane vis");
        return;
    };
    let Ok((box_entity, mut node, mut _bg)) = q_box.single_mut() else {
        info!("No tooltip box");
        return;
    };

    let mut text = {
        let mut found: Option<Mut<Text>> = None;
        for (t, parent) in &mut q_text {
            if parent.parent() == box_entity {
                found = Some(t);
                break;
            }
        }
        match found {
            Some(t) => t,
            None => {
                info!("No tooltip text under the hover box");
                node.display = Display::None;
                return;
            }
        }
    };

    let (Some(cell), Some(world)) = (hover.cell, hover.world_hit) else {
        node.display = Display::None;
        return;
    };

    if vis == Visibility::Hidden {
        node.display = Display::None;
        return;
    }

    // place near cursor with a small offset; clamp to window bounds
    let Some(cursor) = window.cursor_position() else {
        node.display = Display::None;
        return;
    };

    let (logits, probabilities, assignment) = Python::attach(|py| {
        let py_obj = grid.arc().read().unwrap().clone_ref(py);
        let grid_ref = py_obj.borrow(py);
        let idx = (cell.y * grid_ref.width as u32 + cell.x) as usize;
        if idx >= grid_ref.grid.len() {
            // return default-shaped values: logits tuple, probabilities tuple, no assignment
            return ((-1.0, -1.0, -1.0, -1.0), (-1.0, -1.0, -1.0, -1.0), None);
        }
        let entry = &grid_ref.grid[idx];
        let logits = (
            entry.logit_free,
            entry.logit_wall,
            entry.logit_flag,
            entry.logit_capture_point,
        );
        let probs = entry.probabilities();
        let assign = entry.assignment;

        (logits, probs, assign)
    });

    // tweak these if you change box size
    const OFFSET: Vec2 = Vec2::new(0.0, 0.0);
    const BOX_W: f32 = 320.0; // assumed width for clamping
    const BOX_H: f32 = 120.0; // assumed height for clamping

    let mut x = cursor.x + OFFSET.x;
    let mut y = cursor.y + OFFSET.y;

    if x + BOX_W > window.width() - 4.0 {
        x = (window.width() - 4.0) - BOX_W;
    }
    if y + BOX_H > window.height() - 4.0 {
        y = (window.height() - 4.0) - BOX_H;
    }

    node.left = Val::Px(x.max(4.0));
    node.top = Val::Px(y.max(4.0));
    node.display = Display::Grid;

    text.0 = format!(
        "{}\n\
         Cell:       ({},{})\n\
         World:      ({:.2},{:.2})\n\
         Assignment: {}\n\
         Free:       {:.2} ({:.2})\n\
         Wall:       {:.2} ({:.2})\n\
         Flag:       {:.2} ({:.2})\n\
         CP:         {:.2} ({:.2})",
        T::name(),
        cell.x,
        cell.y,
        world.x,
        world.z,
        assignment
            .map(|e| format!("{}", e))
            .unwrap_or("None".to_string()),
        probabilities.0,
        logits.0,
        probabilities.1,
        logits.1,
        probabilities.2,
        logits.2,
        probabilities.3,
        logits.3,
    );
}
