use avian3d::debug_render::PhysicsDebugPlugin;
use avian3d::prelude::*;
use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin, input::common_conditions::input_just_pressed,
    prelude::*,
};
use bevy_inspector_egui::{
    DefaultInspectorConfigPlugin, bevy_egui::EguiPlugin, quick::WorldInspectorPlugin,
};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        // Add diagnostics.
        app.add_plugins((
            PhysicsDiagnosticsPlugin,
            PhysicsDiagnosticsUiPlugin,
            PhysicsDebugPlugin,
            EguiPlugin::default(),
            WorldInspectorPlugin::new(),
            FrameTimeDiagnosticsPlugin::default(),
        ));

        // Configure the default physics diagnostics UI.
        app.insert_resource(PhysicsDiagnosticsUiSettings {
            enabled: false,
            ..default()
        });

        // Spawn text instructions for keybinds.
        app.add_systems(Startup, setup_key_instructions);

        // Add systems for toggling the diagnostics UI and pausing and stepping the simulation.
        app.add_systems(
            Update,
            (
                toggle_diagnostics_ui.run_if(input_just_pressed(KeyCode::KeyU)),
                toggle_paused.run_if(input_just_pressed(KeyCode::KeyP)),
                step.run_if(physics_paused.and(input_just_pressed(KeyCode::Enter))),
                draw_axes,
            ),
        );
    }
}

fn toggle_diagnostics_ui(mut settings: ResMut<PhysicsDiagnosticsUiSettings>) {
    settings.enabled = !settings.enabled;
}

fn physics_paused(time: Res<Time<Physics>>) -> bool {
    time.is_paused()
}

fn toggle_paused(mut time: ResMut<Time<Physics>>) {
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

/// Advances the physics simulation by one `Time<Fixed>` time step.
fn step(mut physics_time: ResMut<Time<Physics>>, fixed_time: Res<Time<Fixed>>) {
    physics_time.advance_by(fixed_time.delta());
}

fn setup_key_instructions(mut commands: Commands) {
    commands.spawn((
        Text::new("U: Diagnostics UI | P: Pause/Unpause | Enter: Step"),
        TextFont {
            font_size: 10.0,
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

fn draw_axes(mut gizmos: Gizmos) {
    let origin = Vec3::ZERO;
    let length = 1.0;

    gizmos.line(origin, Vec3::X * length, Color::srgb(1.0, 0.0, 0.0)); // X axis
    gizmos.line(origin, Vec3::Y * length, Color::srgb(0.0, 1.0, 0.0)); // Y axis
    gizmos.line(origin, Vec3::Z * length, Color::srgb(0.0, 0.0, 1.0)); // Z axis
}
