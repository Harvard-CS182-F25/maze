use bevy::prelude::*;

#[derive(Resource)]
pub struct AgentGraphicsAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub ghost_material: Handle<StandardMaterial>,
}

impl FromWorld for AgentGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mut mesh = Mesh::from(Cuboid::new(1.0, 2.0, 1.0));
        mesh.translate_by(Vec3::Y);
        let mesh = meshes.add(mesh);

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material: Handle<StandardMaterial> = materials.add(Color::srgb(1.0, 0.1, 0.2));
        let ghost_material: Handle<StandardMaterial> =
            materials.add(Color::srgba(1.0, 0.1, 0.2, 0.5));

        Self {
            mesh,
            material,
            ghost_material,
        }
    }
}
