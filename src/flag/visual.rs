use bevy::prelude::*;

#[derive(Resource)]
pub struct FlagGraphicsAssets {
    pub mesh: Handle<Mesh>,
    pub red_material: Handle<StandardMaterial>,
    pub blue_material: Handle<StandardMaterial>,
}

impl FromWorld for FlagGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mesh = meshes.add(Cylinder::default());

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let red_material = materials.add(Color::srgb(1.0, 0.0, 0.0));
        let blue_material = materials.add(Color::srgb(0.0, 0.0, 1.0));

        Self {
            mesh,
            red_material,
            blue_material,
        }
    }
}

#[derive(Resource)]
pub struct CapturePointGraphicsAssets {
    pub mesh: Handle<Mesh>,
    pub red_material: Handle<StandardMaterial>,
    pub blue_material: Handle<StandardMaterial>,
}

impl FromWorld for CapturePointGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mesh = meshes.add(Torus::new(0.35, 0.75));

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let red_material = materials.add(Color::srgb(1.0, 0.0, 0.0));
        let blue_material = materials.add(Color::srgb(0.0, 0.0, 1.0));

        Self {
            mesh,
            red_material,
            blue_material,
        }
    }
}
