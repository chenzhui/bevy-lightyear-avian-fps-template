use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use leafwing_input_manager::prelude::*;
use crate::vleue::feature::character::movement::CharacterAction;
use crate::vleue::feature::core::settings::{msaa_from_samples, GameSettings};
//region client

pub struct FreeCamClientPlugin;

impl Plugin for FreeCamClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FreeCamCursorLockState>();
        app.add_systems(Startup, (spawn_free_cam, spawn_free_cam_light, spawn_free_cam_ui));
        app.add_systems(Update, (maintain_free_cam_cursor, move_free_cam));
    }
}

#[derive(Resource)]
struct FreeCamCursorLockState {
    locked: bool,
}

fn spawn_free_cam(mut commands: Commands, settings: Res<GameSettings>) {
    let initial_state = FreeCamera::default();
    commands.spawn((
        Name::new("FreeCamera"),
        Camera3d::default(),
        msaa_from_samples(settings.graphics.msaa_samples),
        Transform::from_xyz(0.0, 15.0, 30.0)
            .with_rotation(Quat::from_rotation_y(initial_state.yaw) * Quat::from_rotation_x(initial_state.pitch)),
        initial_state,
        InputMap::default().with_dual_axis(CharacterAction::Look, MouseMove::default()),
        ActionState::<CharacterAction>::default(),
    ));
}

fn spawn_free_cam_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 12000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_free_cam_ui(mut commands: Commands, settings: Res<GameSettings>) {
    if !settings.interface.show_crosshair {
        return;
    }
    // Add crosshair at screen center
    commands.spawn((
        Name::new("FreeCamCrosshair"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(12.0),
            height: Val::Px(12.0),
            margin: UiRect::axes(Val::Px(-6.0), Val::Px(-6.0)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    )).with_children(|parent| {
        parent.spawn((
            Text::new("○"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    });
}

impl Default for FreeCamCursorLockState {
    fn default() -> Self {
        Self {
            locked: true,
        }
    }
}



#[derive(Component)]
pub struct FreeCamera {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for FreeCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.6,
        }
    }
}



fn move_free_cam(time: Res<Time>, keyboard: Res<ButtonInput<KeyCode>>, settings: Res<GameSettings>, cursor_lock_state: Res<FreeCamCursorLockState>, mut query: Query<(&mut Transform, &mut FreeCamera, &ActionState<CharacterAction>)>, ) {
    let Ok((mut transform, mut cam_state, action_state)) = query.single_mut() else { return; };
    let delta = time.delta_secs();

    // 1. Handle rotation
    let look = action_state.axis_pair(&CharacterAction::Look);
    if cursor_lock_state.locked && look.length_squared() > 0.0001 {
        cam_state.yaw -= look.x * settings.free_cam.look_sensitivity;
        cam_state.pitch -= look.y * settings.free_cam.look_sensitivity;
        cam_state.pitch = cam_state.pitch.clamp(-1.5, 1.5);
        transform.rotation = Quat::from_rotation_y(cam_state.yaw) * Quat::from_rotation_x(cam_state.pitch);
    }

    let speed = settings.free_cam.move_speed;
    let mut move_dir = Vec3::ZERO;
    if keyboard.pressed(KeyCode::ArrowUp) {
        move_dir.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        move_dir.z += 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowLeft) {
        move_dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        move_dir.x += 1.0;
    }
    if move_dir.length_squared() > 0.0001 {
        move_dir = move_dir.normalize();
        let forward = transform.forward();
        let right = transform.right();
        let flat_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let flat_right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
        let velocity = (flat_forward * -move_dir.z + flat_right * move_dir.x) * speed * delta;
        transform.translation += velocity;
    }

    // 3. Handle vertical movement (X/C)
    if keyboard.pressed(KeyCode::KeyX) {
        transform.translation.y += speed * delta;
    }
    if keyboard.pressed(KeyCode::KeyC) {
        transform.translation.y -= speed * delta;
    }
}

fn set_cursor_locked(windows: &mut Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, locked: bool) {
    let Ok((window, mut cursor)) = windows.single_mut() else { return; };
    if locked && !window.focused {
        return;
    }
    cursor.grab_mode = if locked { CursorGrabMode::Locked } else { CursorGrabMode::None };
    cursor.visible = !locked;
}

fn maintain_free_cam_cursor(mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, keyboard: Res<ButtonInput<KeyCode>>, mut cursor_lock_state: ResMut<FreeCamCursorLockState>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        cursor_lock_state.locked = !cursor_lock_state.locked;
    }
    set_cursor_locked(&mut windows, cursor_lock_state.locked);
}


//endregion client





