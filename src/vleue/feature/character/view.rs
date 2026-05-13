use crate::vleue::feature::character::movement::{CharacterAimPitch, CharacterMarker};
use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::combat::Health;
use crate::vleue::feature::core::settings::{msaa_from_samples, GameSettings, SettingsUiState};
use crate::vleue::feature::core::state::InGameState;
use avian3d::physics_transform::{Position, Rotation};
use bevy::app::{App, Last, Plugin, Startup, Update};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use lightyear::prelude::{Controlled, Predicted};
use std::collections::HashSet;

const FIRST_PERSON_CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 1.6, -0.18);
const FIRST_PERSON_HIDDEN_NODE_KEYWORDS: &[&str] = &["head", "face", "hair", "eye", "teeth", "lash", "brow"];
const HEALTH_BAR_WIDTH: f32 = 260.0;
const HEALTH_BAR_HEIGHT: f32 = 22.0;

#[derive(Component, Default)]
pub struct FirstPersonCamera;

#[derive(Component)]
struct CrosshairRoot;

#[derive(Component)]
struct HealthHudRoot;

#[derive(Component)]
struct HealthBarFill;

#[derive(Component)]
struct HealthBarText;

#[derive(Resource, Default)]
struct CursorLockState {
	locked: bool,
}

#[derive(Resource, Default)]
pub struct FpsCursorUiBlockers {
	blockers: HashSet<&'static str>, // Open UI names; extension UI can write here without public code depending on private state types.
}

impl FpsCursorUiBlockers {
	pub fn set(&mut self, key: &'static str, opened: bool) {
		if opened {
			self.blockers.insert(key);
		} else {
			self.blockers.remove(key);
		}
	}

	pub fn any_open(&self) -> bool {
		!self.blockers.is_empty()
	}
}

pub fn fps_cursor_ui_open(ui_blockers: Option<Res<FpsCursorUiBlockers>>, settings_ui_state: Option<Res<SettingsUiState>>) -> bool {
	let private_ui_opened = ui_blockers.map(|blockers| blockers.any_open()).unwrap_or(false);
	let settings_opened = settings_ui_state.map(|state| state.opened).unwrap_or(false);
	private_ui_opened || settings_opened
}

pub fn allow_fps_character_control(ui_blockers: Option<Res<FpsCursorUiBlockers>>, settings_ui_state: Option<Res<SettingsUiState>>) -> bool {
	!fps_cursor_ui_open(ui_blockers, settings_ui_state)
}

//region client

#[derive(Clone)]
pub struct ViewClientPlugin;

impl Plugin for ViewClientPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<CursorLockState>();
		app.init_resource::<FpsCursorUiBlockers>();
		app.add_systems(Startup, init_camera);
		app.add_systems(OnEnter(InGameState::Playing), (lock_cursor_on_enter_game, spawn_crosshair));
		app.add_systems(Update, maintain_fps_cursor.run_if(in_state(InGameState::Playing)));
		app.add_systems(Update, sync_health_hud.run_if(in_state(InGameState::Playing)));
		app.add_systems(OnExit(InGameState::Playing), (unlock_cursor_on_exit_game, cleanup_crosshair, cleanup_health_hud));
		app.add_systems(Last, update_first_person_camera);
		app.add_observer(hide_local_first_person_head_parts);
	}
}

fn init_camera(mut commands: Commands, settings: Res<GameSettings>) {
	let first_person_fov = settings.camera.first_person_fov_degrees.to_radians();
	commands.spawn((
		Camera3d::default(),
		Projection::Perspective(PerspectiveProjection { // View distance
			fov: first_person_fov,
			..default()
		}),
		msaa_from_samples(settings.graphics.msaa_samples),
		Transform::from_translation(FIRST_PERSON_CAMERA_OFFSET),
		FirstPersonCamera,
		RenderLayers::from_layers(&[0]),
	));
	commands.spawn((
		DirectionalLight {
			shadows_enabled: true,
			illuminance: 12000.0,
			..default()
		},
		Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
	));
}


fn update_first_person_camera(mut commands: Commands, mut camera_query: Query<(Entity, &mut Transform, Option<&ChildOf>), With<FirstPersonCamera>>, character_query: Query<(Entity, &Position, &Rotation, &CharacterAimPitch), (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>), >, ) {
	let Ok((camera_entity, mut camera_transform, camera_parent)) = camera_query.single_mut() else { return; };
	let Ok((character_entity, position, _rotation, aim_pitch)) = character_query.single() else { return; };
	if camera_parent.map(|parent| parent.parent()) != Some(character_entity) {
		commands.entity(camera_entity).set_parent_in_place(character_entity);
	}
	// Push camera slightly forward from head center to avoid first-person camera falling inside character face.
	camera_transform.translation = FIRST_PERSON_CAMERA_OFFSET;
	camera_transform.rotation = Quat::from_rotation_x(aim_pitch.0);// 2. [Core fix]: Camera as child node only needs to handle vertical pitch angle.
	let _ = position;
}

fn hide_local_first_person_head_parts(scene_ready: On<SceneInstanceReady>, mut commands: Commands, local_players: Query<(), (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>)>, children: Query<&Children>, names: Query<&Name>, ) {
	if local_players.get(scene_ready.entity).is_err() {
		return;
	}
	debug!("[view] SceneInstanceReady for local player entity={:?}", scene_ready.entity);

	let mut hidden_nodes = Vec::new();
	for child in children.iter_descendants(scene_ready.entity) {
		let Ok(name) = names.get(child) else { continue; };
		let lowered = name.as_str().to_lowercase();
		if FIRST_PERSON_HIDDEN_NODE_KEYWORDS.iter().any(|keyword| lowered.contains(keyword)) {
			commands.entity(child).insert(Visibility::Hidden);
			hidden_nodes.push(format!("{:?}:{}", child, name.as_str()));
		}
	}
	if hidden_nodes.is_empty() {
		debug!("[view] local player entity={:?} scene ready, but no head-part nodes matched hide keywords", scene_ready.entity);
	} else {
		debug!("[view] local player entity={:?} hid {} nodes: {}", scene_ready.entity, hidden_nodes.len(), hidden_nodes.join(", "));
	}
}

//region Lock view section

fn set_cursor_locked(windows: &mut Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, locked: bool,) {
	let Ok((window, mut cursor)) = windows.single_mut() else { return; };
	if locked && !window.focused {
		return;
	}
	cursor.grab_mode = if locked {
		CursorGrabMode::Locked
	} else {
		CursorGrabMode::None
	};
	cursor.visible = !locked;
}

fn lock_cursor_on_enter_game(mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, mut cursor_lock_state: ResMut<CursorLockState>, ) {
	cursor_lock_state.locked = true;
	set_cursor_locked(&mut windows, true);
}

fn unlock_cursor_on_exit_game(mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, mut cursor_lock_state: ResMut<CursorLockState>, ) {
	cursor_lock_state.locked = false;
	set_cursor_locked(&mut windows, false);
}
fn maintain_fps_cursor(mut windows: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>, cursor_lock_state: Res<CursorLockState>, ui_blockers: Res<FpsCursorUiBlockers>, settings_ui_state: Option<Res<SettingsUiState>>, ) {
	let ui_opened = ui_blockers.any_open() || settings_ui_state.map(|s| s.opened).unwrap_or(false);
	let effective_locked = cursor_lock_state.locked && !ui_opened; // Shooter-style lock: combat locks by default, any in-game UI releases it.
	set_cursor_locked(&mut windows, effective_locked);
}

fn spawn_crosshair(mut commands: Commands, crosshair_query: Query<Entity, With<CrosshairRoot>>, settings: Res<GameSettings>) {
	if !crosshair_query.is_empty() {
		return;
	}
	if !settings.interface.show_crosshair {
		return;
	}
	commands.spawn((
		Name::new("fps_crosshair"),
		CrosshairRoot,
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

fn cleanup_crosshair(mut commands: Commands, crosshair_query: Query<Entity, With<CrosshairRoot>>) {
	for entity in &crosshair_query {
		commands.entity(entity).despawn();
	}
}


fn sync_health_hud(player_query: Query<&Health, (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>)>, mut fill_query: Query<(&mut Node, &mut BackgroundColor), With<HealthBarFill>>, mut text_query: Query<&mut Text, With<HealthBarText>>) { // Sync HUD health bar width and text using local player.
	let Ok(health) = player_query.single() else { return; };
	let ratio = if health.max > 0.0 { (health.current / health.max).clamp(0.0, 1.0) } else { 0.0 };
	let width = ((HEALTH_BAR_WIDTH - 6.0) * ratio).max(0.0);
	if let Ok((mut node, mut color)) = fill_query.single_mut() {
		node.width = Val::Px(width);
		color.0 = if ratio > 0.5 {
			Color::srgb(0.18, 0.72, 0.24)
		} else if ratio > 0.25 {
			Color::srgb(0.88, 0.64, 0.16)
		} else {
			Color::srgb(0.81, 0.18, 0.18)
		};
	}
	if let Ok(mut text) = text_query.single_mut() {
		*text = Text::new(format!("{:.0} / {:.0}", health.current, health.max));
	}
}

fn cleanup_health_hud(mut commands: Commands, hud_query: Query<Entity, With<HealthHudRoot>>) { // Clear health bar HUD when exiting first-person match.
	for entity in &hud_query {
		commands.entity(entity).despawn();
	}
}

//endregion Lock view section


//endregion client
