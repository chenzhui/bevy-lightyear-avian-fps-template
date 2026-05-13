use avian3d::physics_transform::Rotation;
use avian3d::prelude::forces::ForcesItem;
use avian3d::prelude::*;
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use crate::vleue::feature::character::view::{allow_fps_character_control, fps_cursor_ui_open};
use crate::vleue::feature::VleueSide;
use crate::vleue::feature::combat::{LifeState, LifeStatus};
use crate::vleue::feature::core::settings::{GameSettings, InputBinding, PlayerKeybinds};

const MAX_SPEED: f32 = 5.0; // Movement speed.
const MAX_ACCELERATION: f32 = 20.0; // Acceleration rate.
const MOVE_INPUT_DEADZONE_SQUARED: f32 = 0.01; // Movement input deadzone squared, filters joystick drift or tiny input errors.
const STOP_ACCELERATION_MULTIPLIER: f32 = 2.0; // Deceleration multiplier when releasing movement keys, makes stopping cleaner than starting.
const HARD_STOP_SPEED_SQUARED: f32 = 0.001; // Force velocity to zero below this horizontal speed when stopping, avoids residual sliding with zero friction.
const JUMP_IMPULSE: f32 = 5.0; // Upward linear impulse applied on jump, determines initial jump velocity.
pub const CHARACTER_COLLIDER_RADIUS: f32 = 0.35; // Player movement cylinder collider radius, stays close to body but avoids edge clipping.
pub const CHARACTER_COLLIDER_HEIGHT: f32 = 1.5; // Player movement cylinder collider height, character origin at foot center.
pub const CHARACTER_GROUND_PROBE_START_HEIGHT: f32 = 0.1; // Ground check starts slightly above foot level, avoids ray origin buried in ground.
pub const JUMP_GROUND_CHECK_DISTANCE: f32 = 0.2; // Maximum downward ray distance for jump ground check, covers tiny gap between foot and ground.
pub const JUMP_VERTICAL_VELOCITY_EPSILON: f32 = 0.2; // Allowed vertical velocity error when jumping, avoids re-checking jump eligibility right after leaving ground.
pub const WORLD_LAYER: LayerMask = LayerMask(1 << 0); // World static collision layer, ground and scene obstacles go here.
pub const PLAYER_LAYER: LayerMask = LayerMask(1 << 1); // Player movement collision layer, only blocks character vs world.
pub const HITBOX_LAYER: LayerMask = LayerMask(1 << 2); // Hitbox query layer, reserved for shooting hit detection.


#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct CharacterMarker; // Movable character marker.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default)] pub struct CharacterAimPitch(pub f32); // First-person view pitch angle.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default)] pub struct CharacterYaw(pub f32); // Absolute yaw angle.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect, Serialize, Deserialize)] pub enum CharacterAction { MoveUp, MoveDown, MoveLeft, MoveRight, Jump, Look, Interact, Attack, Shoot, SkillQ, UseMedkit, UseTeleportScroll, }
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)] pub enum CharacterHitboxKind { Head, Body, } // Character hitbox type, used for body-part-specific damage calculation.

impl Actionlike for CharacterAction {
    fn input_control_kind(&self) -> InputControlKind {
        match self {
            Self::Look => InputControlKind::DualAxis,
            Self::MoveUp | Self::MoveDown | Self::MoveLeft | Self::MoveRight | Self::Jump | Self::Interact | Self::Attack | Self::Shoot | Self::SkillQ | Self::UseMedkit | Self::UseTeleportScroll => InputControlKind::Button,
        }
    }
}

#[derive(Bundle)]
pub struct CharacterPhysicsBundle {
    rigid_body: RigidBody,
    locked_axes: LockedAxes,
    friction: Friction,
}

impl Default for CharacterPhysicsBundle {
    fn default() -> Self {
        Self {
            rigid_body: RigidBody::Dynamic,
            locked_axes: LockedAxes::default().lock_rotation_x().lock_rotation_y().lock_rotation_z(),
            friction: Friction::new(0.0)  // Rigid body friction is 0.0, provides no friction itself. Intuitively "very slippery".
                .with_combine_rule(CoefficientCombine::Min),  // Friction takes the smaller of both sides, so movement is entirely controlled by user input.
        }
    }
}

pub struct MovementPlugin {
    pub side: VleueSide, // Character movement entry point; registers synced components and installs prediction/authority systems by side.
}

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MovementShaderPlugin);
        match self.side {
            VleueSide::Client => app.add_plugins(MovementClientPlugin),
            VleueSide::Server => app.add_plugins(MovementServerPlugin),
        };
    }
}


//region shader

#[derive(Clone)]
pub struct MovementShaderPlugin;

impl Plugin for MovementShaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (update_character_aim_pitch, update_character_rotation).run_if(allow_fps_character_control), );
        app.register_component::<CharacterMarker>();
        app.register_component::<CharacterAimPitch>().add_prediction().add_should_rollback(aim_pitch_should_rollback);
        app.register_component::<CharacterYaw>().add_prediction().add_should_rollback(aim_yaw_should_rollback);
    }
}

fn aim_pitch_should_rollback(this: &CharacterAimPitch, that: &CharacterAimPitch) -> bool {
    (this.0 - that.0).abs() >= 0.001
}

fn aim_yaw_should_rollback(this: &CharacterYaw, that: &CharacterYaw) -> bool {
    (this.0 - that.0).abs() >= 0.001
}



pub fn update_character_rotation(mut query: Query<(&mut CharacterYaw, &mut Rotation, &ActionState<CharacterAction>), With<CharacterMarker>>, ) {
    for (mut yaw, mut rotation, action_state) in &mut query { // Update character facing direction using mouse horizontal input (absolute yaw angle overrides physics rotation).
        let yaw_delta = action_state.axis_pair(&CharacterAction::Look).x;
        if yaw_delta.abs() > 0.0001 {
            yaw.0 -= yaw_delta;
            yaw.0 = yaw.0.rem_euclid(std::f32::consts::TAU);
        }
        rotation.0 = Quat::from_rotation_y(yaw.0);
    }
}

pub fn update_character_aim_pitch(mut query: Query<(&mut CharacterAimPitch, &ActionState<CharacterAction>), With<CharacterMarker>>) {
    for (mut aim_pitch, action_state) in &mut query { // Update pitch angle using Look input already processed by local settings.
        let pitch_delta = action_state.axis_pair(&CharacterAction::Look).y;
        if pitch_delta.abs() <= 0.0001 {
            continue;
        }
        aim_pitch.0 = (aim_pitch.0 + pitch_delta)
            .clamp(-70.0_f32.to_radians(), 70.0_f32.to_radians()); // Clamp within 70 degrees.
    }
}

pub fn character_is_grounded(entity: Entity, position: Vec3, linear_velocity: Vec3, spatial_query: &SpatialQuery) -> bool {
    let ray_cast_origin = position + Vec3::Y * CHARACTER_GROUND_PROBE_START_HEIGHT;
    let is_nearly_grounded = linear_velocity.y.abs() <= JUMP_VERTICAL_VELOCITY_EPSILON;
    let ground_filter = SpatialQueryFilter::from_mask(WORLD_LAYER).with_excluded_entities([entity]);
    is_nearly_grounded && spatial_query.cast_ray(ray_cast_origin, Dir3::NEG_Y, JUMP_GROUND_CHECK_DISTANCE, true, &ground_filter, ).is_some()
}


pub fn apply_character_action(entity: Entity, _mass: &ComputedMass, rotation: &Rotation, time: &Res<Time>, spatial_query: &SpatialQuery, action_state: &ActionState<CharacterAction>, life_state: &LifeState, mut forces: ForcesItem, ) {
    // Dead state disables all actions.
    if life_state.status == LifeStatus::Dead {
        let velocity = forces.linear_velocity_mut();
        velocity.x = 0.0;
        velocity.z = 0.0;
        return;
    }

    // Only alive state can jump.
    if life_state.status == LifeStatus::Alive && action_state.just_pressed(&CharacterAction::Jump) {
        let linear_velocity = forces.linear_velocity();
        if character_is_grounded(entity, forces.position().0, linear_velocity, spatial_query) {
            forces.apply_linear_impulse(Vec3::new(0.0, JUMP_IMPULSE, 0.0));
        }
    }


    let mut input_dir = movement_input_dir(action_state); // Normalize user movement direction.
    if input_dir.length_squared() < MOVE_INPUT_DEADZONE_SQUARED {
        input_dir = Vec2::ZERO;
    }
    let world_move_dir = rotation.0 * Vec3::new(input_dir.x, 0.0, -input_dir.y); // Get world direction.
    let move_dir = Vec3::new(world_move_dir.x, 0.0, world_move_dir.z); // Disallow vertical movement, theoretically there won't be any vertical component.
    let linear_velocity = forces.linear_velocity();
    let ground_linear_velocity = Vec3::new(linear_velocity.x, 0.0, linear_velocity.z);

    // Adjust max speed based on state: downed has only 20% speed.
    let current_max_speed = if life_state.status == LifeStatus::Downed { MAX_SPEED * 0.2 } else { MAX_SPEED };

    let desired_ground_linear_velocity = move_dir * current_max_speed; // Calculate desired velocity.
    let is_stopping = move_dir.length_squared() == 0.0;
    let acceleration_rate = if is_stopping { MAX_ACCELERATION * STOP_ACCELERATION_MULTIPLIER } else { MAX_ACCELERATION };
    let max_velocity_delta_per_tick = acceleration_rate * time.delta_secs();
    let new_ground_linear_velocity = ground_linear_velocity.move_towards(desired_ground_linear_velocity, max_velocity_delta_per_tick); // Calculate required acceleration.
    let velocity = forces.linear_velocity_mut(); // Directly control horizontal speed, Y axis left for gravity and jump.
    if is_stopping && new_ground_linear_velocity.length_squared() < HARD_STOP_SPEED_SQUARED {
        velocity.x = 0.0;
        velocity.z = 0.0;
    } else {
        velocity.x = new_ground_linear_velocity.x;
        velocity.z = new_ground_linear_velocity.z;
    }
}


//endregion shader




//region server
#[derive(Clone)]
pub struct MovementServerPlugin;

impl Plugin for MovementServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, ( handle_character_actions_server,), );
    }
}

pub fn movement_input_dir(action_state: &ActionState<CharacterAction>) -> Vec2 {
    let mut input_dir = Vec2::ZERO;
    if action_state.pressed(&CharacterAction::MoveLeft) {
        input_dir.x -= 1.0;
    }
    if action_state.pressed(&CharacterAction::MoveRight) {
        input_dir.x += 1.0;
    }
    if action_state.pressed(&CharacterAction::MoveUp) {
        input_dir.y += 1.0;
    }
    if action_state.pressed(&CharacterAction::MoveDown) {
        input_dir.y -= 1.0;
    }
    input_dir.clamp_length_max(1.0)
}

fn handle_character_actions_server(time: Res<Time>, spatial_query: Option<SpatialQuery>, mut query: Query<     (Entity, &ComputedMass, &Rotation, &ActionState<CharacterAction>, &LifeState, Forces,), With<CharacterMarker>, >, ) { // Server authoritative side executes the same movement logic.
    let Some(spatial_query) = spatial_query else { return; };
    for (entity, mass, rotation, action_state, life_state, forces) in &mut query {
        apply_character_action(entity, mass, rotation, &time, &spatial_query, action_state, life_state, forces, );
    }
}

//endregion server

//region client

#[derive(Clone)]
pub struct MovementClientPlugin;

impl Plugin for MovementClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, clear_local_character_input_when_ui_open.run_if(fps_cursor_ui_open));
        app.add_systems(FixedPreUpdate, clear_local_character_input_when_ui_open.run_if(fps_cursor_ui_open));
        app.add_systems(FixedUpdate, (handle_character_actions_client,).run_if(allow_fps_character_control), );
        app.add_systems(Update, sync_character_input_map);
        app.add_observer(setup_predicted_player);

    }
}

fn clear_local_character_input_when_ui_open(mut query: Query<&mut ActionState<CharacterAction>, (With<CharacterMarker>, With<Controlled>, With<Predicted>)>) {
    for mut action_state in &mut query {
        *action_state = ActionState::default();
    }
}


fn handle_character_actions_client(time: Res<Time>, spatial_query: Option<SpatialQuery>, timeline: Res<LocalTimeline>,
    mut query: Query<(Entity, &ComputedMass, &Rotation, &ActionState<CharacterAction>, &LifeState, Forces,), (With<CharacterMarker>, With<Controlled>, With<Predicted>), >,
) {
    // Client prediction side executes the same movement logic.
    let Some(spatial_query) = spatial_query else { return; };
    let _tick = timeline.tick();
    for (entity, mass, rotation, action_state, life_state, forces) in &mut query {
        apply_character_action(entity, mass, rotation, &time, &spatial_query, action_state, life_state, forces, );
    }
}


fn setup_predicted_player(trigger: On<Add, (Predicted, CharacterMarker)>, mut commands: Commands, predicted_query: Query<Entity, (With<Predicted>, With<CharacterMarker>)>, keybinds: Res<PlayerKeybinds>, settings: Res<GameSettings>, ) { // Add input mapping and physics body for local predicted player.
    let entity = trigger.entity;
    if predicted_query.get(entity).is_err() {
        return;
    }
    commands.entity(entity).insert((
        CharacterAimPitch::default(),
        CharacterPhysicsBundle::default(),
        build_character_input_map(&keybinds, &settings),
    ));
}

fn sync_character_input_map(settings: Res<GameSettings>, keybinds: Res<PlayerKeybinds>, mut query: Query<&mut InputMap<CharacterAction>, (With<CharacterMarker>, With<Controlled>, With<Predicted>)>) {
    if !settings.is_changed() && !keybinds.is_changed() {
        return;
    }
    for mut input_map in &mut query {
        *input_map = build_character_input_map(&keybinds, &settings);
    }
}

fn build_character_input_map(keybinds: &PlayerKeybinds, settings: &GameSettings) -> InputMap<CharacterAction> {
    let mut input_map = InputMap::default();
    insert_button_binding(&mut input_map, CharacterAction::MoveUp, keybinds.move_up);
    insert_button_binding(&mut input_map, CharacterAction::MoveDown, keybinds.move_down);
    insert_button_binding(&mut input_map, CharacterAction::MoveLeft, keybinds.move_left);
    insert_button_binding(&mut input_map, CharacterAction::MoveRight, keybinds.move_right);
    insert_button_binding(&mut input_map, CharacterAction::Jump, keybinds.jump);
    insert_button_binding(&mut input_map, CharacterAction::Interact, keybinds.interact);
    let look_input = if settings.camera.invert_y {
        MouseMove::default().sensitivity(settings.camera.mouse_sensitivity)
    } else {
        MouseMove::default().sensitivity(settings.camera.mouse_sensitivity).inverted_y()
    };
    input_map.insert_dual_axis(CharacterAction::Look, look_input);
    insert_button_binding(&mut input_map, CharacterAction::Attack, keybinds.attack);
    insert_button_binding(&mut input_map, CharacterAction::Shoot, keybinds.shoot);
    insert_button_binding(&mut input_map, CharacterAction::SkillQ, keybinds.skill_q);
    insert_button_binding(&mut input_map, CharacterAction::UseMedkit, keybinds.use_medkit);
    insert_button_binding(&mut input_map, CharacterAction::UseTeleportScroll, keybinds.use_teleport_scroll);
    input_map
}

fn insert_button_binding(input_map: &mut InputMap<CharacterAction>, action: CharacterAction, binding: InputBinding) {
    match binding {
        InputBinding::Keyboard(key) => { input_map.insert(action, key); }
        InputBinding::Mouse(button) => { input_map.insert(action, button); }
    }
}


//endregion client







