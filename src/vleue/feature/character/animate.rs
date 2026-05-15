use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use leafwing_input_manager::prelude::ActionState;
use lightyear::prelude::PredictionRegistrationExt;
use lightyear_replication::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use avian3d::physics_transform::Position;
use avian3d::prelude::SpatialQuery;
use crate::vleue::feature::VleueSide;
use crate::vleue::feature::character::input::{CharacterAttackIntent, CharacterShootIntent};
use crate::vleue::feature::character::movement::{character_is_grounded, movement_input_dir, CharacterAction, CharacterMarker};

pub const CHARACTER_MODEL_PATH: &str = "girl.glb";
const RUN_ENTER_SPEED: f32 = 0.35;
const RUN_EXIT_SPEED: f32 = 0.15;
const MELEE_ANIM_LOCK: f32 = 0.42;
const SHOOT_ANIM_LOCK: f32 = 0.18;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum CharacterAnim {
	#[default]
	None, // No action, current temporary non-animated model stays static by default.
	Idle, // Idle loop.
	Run, // Run loop.
	Jump, // Jump non-loop.
	Attack, // Melee attack non-loop.
	Shoot, // Shoot non-loop.
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct CharacterAnimState {
	pub current: CharacterAnim, // Lightweight synced animation state, currently syncs movement and combat animation selection.
	pub animation_lock: Option<(CharacterAnim, f32)>, // Animation-owned lock, keeps short attack/shoot clips from being immediately overridden by movement.
}

impl CharacterAnimState {
	pub fn locked_animation(&self) -> Option<CharacterAnim> {
		self.animation_lock.and_then(|(anim, t)| (t > 0.0).then_some(anim))
	}
}

#[derive(Component)]
struct CharacterAnimationDriver {
	owner: Entity, // The character entity this animation player actually drives.
	last: Option<CharacterAnim>, // Records last played animation state, avoids per-frame redundant switching.
}

#[derive(Resource)]
struct CharacterAnimationAssets {
	gltf: Handle<Gltf>, // Character glTF resource handle, used to read animation clips.
	graph: Option<Handle<AnimationGraph>>, // Built animation graph resource handle.
	idle: Option<AnimationNodeIndex>, // Idle animation node index in the graph.
	run: Option<AnimationNodeIndex>, // Run animation node index in the graph.
	jump: Option<AnimationNodeIndex>, // Jump animation node index in the graph.
	attack: Option<AnimationNodeIndex>, // Attack animation node index in the graph.
	shoot: Option<AnimationNodeIndex>, // Shoot animation node index in the graph.
}

pub struct AnimationPlugin {
	pub side: VleueSide, // Character animation entry point; registers synced data and installs side-specific systems.
}

impl Plugin for AnimationPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(AnimationShaderPlugin);
		match self.side {
			VleueSide::Client => app.add_plugins(AnimationClientPlugin),
			VleueSide::Server => app.add_plugins(AnimationServerPlugin),
		};
	}
}


//region shader

pub struct AnimationShaderPlugin; // Register animation sync components.

impl Plugin for AnimationShaderPlugin {
	fn build(&self, app: &mut App) {
		app.register_component::<CharacterAnimState>().add_prediction().add_should_rollback(anim_state_should_rollback);
	}
}

fn anim_state_should_rollback(this: &CharacterAnimState, that: &CharacterAnimState) -> bool {
	this.current != that.current
}
//endregion shader

//region server

pub struct AnimationServerPlugin; // Server decides target animation based on movement and combat state.
impl Plugin for AnimationServerPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(FixedUpdate, update_character_animation_state);
	}
}

fn update_character_animation_state(spatial_query: SpatialQuery, time: Res<Time>, mut attack_events: MessageReader<CharacterAttackIntent>, mut shoot_events: MessageReader<CharacterShootIntent>, mut query: Query<(Entity, &Position, &LinearVelocity, &ActionState<CharacterAction>, &mut CharacterAnimState), With<CharacterMarker>>, ) {
	let dt = time.delta_secs();
	let attack_entities: Vec<Entity> = attack_events.read().map(|event| event.entity).collect();
	let shoot_entities: Vec<Entity> = shoot_events.read().map(|event| event.entity).collect();
	for (entity, position, velocity, action_state, mut anim_state) in &mut query {
		if let Some((anim, t)) = anim_state.animation_lock {
			let next = (t - dt).max(0.0);
			anim_state.animation_lock = (next > 0.0).then_some((anim, next));
		}
		if attack_entities.contains(&entity) {
			anim_state.animation_lock = Some((CharacterAnim::Attack, MELEE_ANIM_LOCK));
		}
		if shoot_entities.contains(&entity) {
			anim_state.animation_lock = Some((CharacterAnim::Shoot, SHOOT_ANIM_LOCK));
		}
		let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
		let move_input = movement_input_dir(action_state).length();
		let wants_move = move_input > 0.1;
		let is_grounded = character_is_grounded(entity, position.0, velocity.0, &spatial_query);
		let locked_anim = anim_state.locked_animation();
		anim_state.current = match anim_state.current {
			_ if !is_grounded => CharacterAnim::Jump,
			_ if matches!(locked_anim, Some(CharacterAnim::Shoot)) => CharacterAnim::Shoot,
			_ if matches!(locked_anim, Some(CharacterAnim::Attack)) => CharacterAnim::Attack,
			CharacterAnim::None if wants_move && horizontal_speed >= RUN_ENTER_SPEED => CharacterAnim::Run,
			CharacterAnim::Idle if wants_move && horizontal_speed >= RUN_ENTER_SPEED => CharacterAnim::Run,
			CharacterAnim::Run if wants_move && horizontal_speed > RUN_EXIT_SPEED => CharacterAnim::Run,
			_ => CharacterAnim::None,
		};
	}
}
//endregion server

//region client

pub struct AnimationClientPlugin; // Client responsible for loading animation graph and driving model playback.
impl Plugin for AnimationClientPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, init_character_animation_assets);
		app.add_systems(Update, (prepare_character_animation_graph, update_character_model_animation));
		app.add_observer(bind_animation_player_when_scene_ready);
	}
}

fn init_character_animation_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
	commands.insert_resource(CharacterAnimationAssets {
		gltf: asset_server.load(CHARACTER_MODEL_PATH),
		graph: None,
		idle: None,
		run: None,
		jump: None,
		attack: None,
		shoot: None,
	});
}

fn prepare_character_animation_graph(mut assets_state: ResMut<CharacterAnimationAssets>, gltf_assets: Res<Assets<Gltf>>, mut graphs: ResMut<Assets<AnimationGraph>>) {
	if assets_state.graph.is_some() {
		return;
	}
	let Some(gltf) = gltf_assets.get(&assets_state.gltf) else {
		return;
	};
	let pick_named = |keys: &[&str]| -> Option<Handle<AnimationClip>> {
		for key in keys {
			if let Some((_, handle)) = gltf.named_animations.iter().find(|(name, _)| name.to_lowercase().contains(&key.to_lowercase())) {
				return Some(handle.clone());
			}
		}
		None
	};
	let idle_clip = pick_named(&["idle"]).or_else(|| gltf.animations.first().cloned());
	let run_clip = pick_named(&["firstrun", "run", "walk"]).or_else(|| gltf.animations.get(1).cloned()).or_else(|| idle_clip.clone());
	let jump_clip = pick_named(&["jump"]).or_else(|| gltf.animations.get(3).cloned()).or_else(|| idle_clip.clone());
	let attack_clip = pick_named(&["attack"]).or_else(|| gltf.animations.first().cloned()).or_else(|| idle_clip.clone());
	let shoot_clip = pick_named(&["shoot"]).or_else(|| gltf.animations.get(4).cloned()).or_else(|| attack_clip.clone()).or_else(|| idle_clip.clone());
	let (Some(idle_clip), Some(run_clip), Some(jump_clip), Some(attack_clip), Some(shoot_clip)) = (idle_clip, run_clip, jump_clip, attack_clip, shoot_clip) else {
		return;
	};
	let mut graph = AnimationGraph::new();
	let idle = graph.add_clip(idle_clip, 1.0, graph.root);
	let run = graph.add_clip(run_clip, 1.0, graph.root);
	let jump = graph.add_clip(jump_clip, 1.0, graph.root);
	let attack = graph.add_clip(attack_clip, 1.0, graph.root);
	let shoot = graph.add_clip(shoot_clip, 1.0, graph.root);
	let graph_handle = graphs.add(graph);
	assets_state.graph = Some(graph_handle);
	assets_state.idle = Some(idle);
	assets_state.run = Some(run);
	assets_state.jump = Some(jump);
	assets_state.attack = Some(attack);
	assets_state.shoot = Some(shoot);
}

fn bind_animation_player_when_scene_ready(scene_ready: On<SceneInstanceReady>, mut commands: Commands, assets_state: Res<CharacterAnimationAssets>, roots: Query<(), With<CharacterMarker>>, children: Query<&Children>, players: Query<(), With<AnimationPlayer>>, ) {
	if roots.get(scene_ready.entity).is_err() {
		return;
	}
	debug!("[animate] SceneInstanceReady for character entity={:?}", scene_ready.entity);
	let (Some(graph), Some(_idle)) = (assets_state.graph.clone(), assets_state.idle) else {
		debug!("[animate] character entity={:?} has no animation graph, static model is allowed", scene_ready.entity);
		return;
	};
	let mut bound_player_count = 0;
	for child in children.iter_descendants(scene_ready.entity) {
		if players.get(child).is_ok() {
			commands.entity(child).insert((
				AnimationGraphHandle(graph.clone()),
				AnimationTransitions::new(),
				CharacterAnimationDriver {
					owner: scene_ready.entity,
					last: None,
				},
			));
			bound_player_count += 1;
		}
	}
	debug!("[animate] character entity={:?} bound AnimationPlayer count={}", scene_ready.entity, bound_player_count);
}

fn update_character_model_animation(assets_state: Res<CharacterAnimationAssets>, character_query: Query<&CharacterAnimState, With<CharacterMarker>>, mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions, &mut CharacterAnimationDriver)>, ) {
	let (Some(idle), Some(run), Some(jump), Some(attack), Some(shoot)) = (assets_state.idle, assets_state.run, assets_state.jump, assets_state.attack, assets_state.shoot) else {
		return;
	};
	for (mut player, mut transitions, mut driver) in &mut players {
		let Ok(anim_state) = character_query.get(driver.owner) else {
			continue;
		};
		if driver.last == Some(anim_state.current) {
			continue;
		}
		if anim_state.current == CharacterAnim::None {
			player.stop_all();
			driver.last = Some(anim_state.current);
			continue;
		}
		let (node, repeat) = match anim_state.current {
			CharacterAnim::None => unreachable!("CharacterAnim::None is handled before animation node selection"),
			CharacterAnim::Idle => (idle, true),
			CharacterAnim::Run => (run, true),
			CharacterAnim::Jump => (jump, false),
			CharacterAnim::Attack => (attack, false),
			CharacterAnim::Shoot => (shoot, false),
		};
		let active = transitions.play(&mut player, node, Duration::from_millis(120));
		if repeat {
			active.repeat();
		}
		driver.last = Some(anim_state.current);
	}
}
//endregion client
