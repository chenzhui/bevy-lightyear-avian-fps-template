use crate::vleue::feature::character::animate::CharacterAnimState;
use crate::vleue::feature::character::movement::{CharacterAction, CharacterAimPitch, CharacterHitboxKind, CharacterMarker, CharacterPhysicsBundle, CharacterYaw, CHARACTER_COLLIDER_HEIGHT, CHARACTER_COLLIDER_RADIUS, HITBOX_LAYER, PLAYER_LAYER, WORLD_LAYER};
use crate::vleue::feature::character::view::ViewClientPlugin;
use crate::vleue::feature::VleueSide;
use avian3d::physics_transform::{Position, Rotation};
use avian3d::prelude::{AngularVelocity, Collider, ColliderDensity, CollisionLayers, LinearVelocity, Sensor};
use bevy::app::{App, Plugin, Update};
use bevy::math::Vec3;
use bevy::prelude::{Add, Component, On, Query, Res, With, Without};
use bevy::prelude::{AssetServer, Commands, Entity, Name, SceneRoot, Transform};
use lightyear::prelude::PeerId;
use lightyear_replication::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};


pub mod movement; // Player movement, jumping, and physics tuning.
pub mod input; // Input intent layer that translates button state into gameplay events.
pub mod free_cam; // Free camera mode.
pub mod view; // First-person view, crosshair, and HUD.
pub mod animate;  // Character animation system.
pub mod types;

pub struct CharacterPlugin {
	pub side: VleueSide,
}


impl Plugin for CharacterPlugin {
	fn build(&self, app: &mut App) {
		let is_free_cam = app.world().get_resource::<crate::vleue::cli::IsFreeCam>().map(|r| r.0).unwrap_or(false); // The character plugin detects whether free camera mode is active.
		app.add_plugins((
			PlayerPlugin { side: self.side },
			input::CharacterInputPlugin { side: self.side },
			animate::AnimationPlugin { side: self.side },
			movement::MovementPlugin { side: self.side },
		)); // The character entry point only composes sub-features; shader/client/server details stay in sub-plugins.
		match self.side {
			VleueSide::Client => {
				if is_free_cam {
					app.add_plugins(free_cam::FreeCamClientPlugin);
				} else {
					app.add_plugins(ViewClientPlugin);
				}
			}
			VleueSide::Server => {
			}
		};
	}
}


#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VleuePlayer; // Player entity marker.

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VleueClientId(pub PeerId); // Player's corresponding network client ID.



#[derive(Clone)]
pub struct PlayerShaderPlugin; // Movement public entry point, attaches physics and sync plugins.

pub struct PlayerPlugin {
	pub side: VleueSide, // Player entity entry point; installs initialization and presentation systems by side.
}

impl Plugin for PlayerPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(PlayerShaderPlugin);
		match self.side {
			VleueSide::Client => app.add_plugins(PlayerClientPlugin),
			VleueSide::Server => app.add_plugins(PlayerServerPlugin),
		};
	}
}

impl Plugin for PlayerShaderPlugin {
	fn build(&self, app: &mut App) {
		// 1. Register input protocol (core missing point)
		app.add_plugins(lightyear::prelude::input::leafwing::InputPlugin::<CharacterAction> {
			config: lightyear::prelude::input::InputConfig::<CharacterAction> {
				rebroadcast_inputs: false,
				..bevy::prelude::default()
			},
		});
		app.register_component::<VleuePlayer>();
		app.register_component::<VleueClientId>();
	}
}

//region server

#[derive(Clone)]
pub struct PlayerServerPlugin;

impl Plugin for PlayerServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(setup_server_player);
    }
}


fn setup_server_player(trigger: On<Add, VleuePlayer>, mut commands: Commands, query: Query<(Option<&Transform>, Option<&Position>), With<VleuePlayer>>, ) { // Server adds essential 3D character components for new player.
    let entity = trigger.entity;
    let Ok((transform, position)) = query.get(entity) else {
        return;
    };
    let spawn_translation = position.map(|position| position.0).or_else(|| transform.map(|transform| transform.translation)).unwrap_or(Vec3::new(0.0, 0., 0.0));
    commands.entity(entity).insert((
        CharacterMarker,
        CharacterAimPitch::default(),
        CharacterYaw::default(),
        CharacterAnimState::default(),
    ));




    commands.entity(entity).insert((

        Position::new(spawn_translation),
        Rotation::default(),
        LinearVelocity::ZERO,
        AngularVelocity::ZERO,
        CharacterPhysicsBundle::default(),
    ));
    add_character_movement_collider(&mut commands, entity);
    add_character_hitboxes(&mut commands, entity);
}


//endregion server

//region client


#[derive(Clone)]
pub struct PlayerClientPlugin;

impl Plugin for PlayerClientPlugin {
    fn build(&self, app: &mut App) {
        // app.add_observer(add_player_visuals);
        app.add_systems(Update, add_player_visuals);
    }
}

fn add_player_visuals(mut commands: Commands, asset_server: Res<AssetServer>, query: Query<Entity, (With<CharacterMarker>, With<VleuePlayer>, Without<SceneRoot>)>, ) {
    for entity in &query {

        commands.entity(entity).insert(Name::new("PlayerModel"));
        add_character_visual(&mut commands, entity, &asset_server);
        add_character_movement_collider(&mut commands, entity);
    }
}


const CHARACTER_SCENE_PATH: &str = "girl.glb#Scene0";

pub fn add_character_visual(commands: &mut Commands, entity: Entity, asset_server: &AssetServer) {
    commands.entity(entity).insert((
        SceneRoot(asset_server.load(CHARACTER_SCENE_PATH)),
        Transform::default(),
    ));
}

//endregion client

pub fn add_character_movement_collider(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Name::new("CharacterMoveCollider"),
            Collider::cylinder(CHARACTER_COLLIDER_RADIUS, CHARACTER_COLLIDER_HEIGHT),
            CollisionLayers::new(PLAYER_LAYER, WORLD_LAYER),
            Transform::from_xyz(0.0, CHARACTER_COLLIDER_HEIGHT * 0.5, 0.0),
        ));
    });
}

pub fn add_character_hitboxes(commands: &mut Commands, entity: Entity) {
    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Name::new("CharacterMoveCollider"),
            CharacterHitboxKind::Body,
            Sensor,
            Collider::cylinder(0.32, 0.95),
            ColliderDensity(0.0),
            CollisionLayers::new(HITBOX_LAYER, 0u32),
            Transform::from_xyz(0.0, 0.95, 0.0),
        ));
        parent.spawn((
            Name::new("CharacterHeadHitbox"),
            CharacterHitboxKind::Head,
            Sensor,
            Collider::sphere(0.18),
            ColliderDensity(0.0),
            CollisionLayers::new(HITBOX_LAYER, 0u32),
            Transform::from_xyz(0.0, 1.62, 0.0),
        ));
    });
}




// pub fn add_character_scene_and_collision(commands: &mut Commands, entity: Entity, asset_server: &AssetServer) {
// 	add_character_visual(commands, entity, asset_server);
// 	add_character_movement_collider(commands, entity);
// 	add_character_hitboxes(commands, entity);
// }
