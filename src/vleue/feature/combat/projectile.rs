use avian3d::physics_transform::{Position, Rotation};
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use lightyear::prelude::{Controlled, Predicted};
use crate::vleue::feature::character::input::CharacterShootIntent;
use crate::vleue::feature::character::movement::{CharacterAimPitch, CharacterHitboxKind, CharacterMarker, HITBOX_LAYER};
use crate::vleue::feature::character::view::FirstPersonCamera;
use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::combat::damage::{apply_damage_per_target, DAMAGE_HUD_DURATION};
use crate::vleue::feature::combat::{CombatLoadout, Health, LifeState, LifeStatus, RecentlyDamaged};
use crate::vleue::feature::combat::types::CombatState;
use crate::vleue::feature::core::connection::GameRoomId;

pub const BULLET_SPEED: f32 = 90.0;/// Bullet flight speed in meters per second.
pub const BULLET_RANGE: f32 = 50.0;/// Maximum bullet travel distance.
pub const BULLET_RADIUS: f32 = 0.045;/// Bullet visual radius.
const BULLET_VISUAL_LENGTH: f32 = 0.36;/// Bullet visual length.
const BULLET_SPARK_DURATION: f32 = 0.18;/// Hit spark duration in seconds.

#[derive(Component)] pub struct CombatBullet {
	pub shooter: Entity,       // Shooter entity, used to ignore self-hits.
	pub room_id: u64,          // Room isolation ID.
	pub velocity: Vec3,        // World-space velocity.
	pub remaining_range: f32,  // Remaining travel distance.
	pub body_damage: f32,      // Body hit damage.
	pub head_damage: f32,      // Head hit damage.
}

#[derive(Component)] pub(crate) struct BulletVisual { velocity: Vec3, remaining_range: f32 }/// Client-local bullet visual.
#[derive(Component)] pub(crate) struct BulletSpark { timer: f32 }/// Bullet hit spark marker.

pub(crate) fn handle_client_shoot_visuals(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, camera_query: Query<(&Camera, &GlobalTransform), With<FirstPersonCamera>>,
	mut shoot_events: MessageReader<CharacterShootIntent>, player_query: Query<(Entity, &Position, &Rotation, &CharacterAimPitch), (With<VleuePlayer>, With<CharacterMarker>, With<Controlled>, With<Predicted>)>,
) {// Client-side response to shoot input, spawning local bullet visuals.
	let Ok((_, _)) = camera_query.single() else { return; };
	let Ok((player_entity, pos, rot, pitch)) = player_query.single() else { return; };
	let shoot_count = shoot_events.read().filter(|event| event.entity == player_entity).count();
	if shoot_count == 0 { return; }

	for _ in 0..shoot_count {
		let Some((origin, forward)) = bullet_spawn_transform(pos.0, rot.0, pitch.0) else { continue; };
		spawn_bullet_visual(&mut commands, &mut meshes, &mut materials, origin, forward);
	}
}

pub(crate) fn spawn_server_bullets(mut commands: Commands, mut shoot_events: MessageReader<CharacterShootIntent>, mut attackers: Query<(Entity, &GameRoomId, &Position, &Rotation, &CharacterAimPitch, &CombatLoadout, &mut CombatState, &LifeState), (With<CharacterMarker>, With<VleuePlayer>)>) {
	let shoot_entities: Vec<Entity> = shoot_events.read().map(|event| event.entity).collect();
	if shoot_entities.is_empty() { return; }
	for (entity, room_id, position, rotation, aim_pitch, loadout, mut state, life) in &mut attackers {
		if !shoot_entities.contains(&entity) || life.status != LifeStatus::Alive || state.shoot_cooldown > 0.0 {
			continue;
		}
		let Some((origin, forward)) = bullet_spawn_transform(position.0, rotation.0, aim_pitch.0) else { continue; };
		state.shoot_cooldown = crate::vleue::feature::combat::damage::SHOOT_COOLDOWN;
		state.animation_lock = Some((crate::vleue::feature::character::animate::CharacterAnim::Shoot, crate::vleue::feature::combat::damage::SHOOT_ANIM_LOCK));
		commands.spawn((
			Name::new("CombatBullet"),
			Transform::from_translation(origin).with_rotation(Quat::from_rotation_arc(Vec3::Z, forward)),
			CombatBullet {
				shooter: entity,
				room_id: room_id.0,
				velocity: forward * BULLET_SPEED,
				remaining_range: BULLET_RANGE,
				body_damage: loadout.shoot_body_damage,
				head_damage: loadout.shoot_head_damage,
			},
		));
	}
}

pub(crate) fn update_server_bullets(mut commands: Commands, time: Res<Time>, spatial_query: SpatialQuery, mut bullets: Query<(Entity, &mut Transform, &mut CombatBullet)>, hitbox_query: Query<(&CharacterHitboxKind, &ChildOf)>, mut target_query: Query<(&mut Health, &mut LifeState, &GameRoomId)>) {
	let dt = time.delta_secs();
	for (bullet_entity, mut transform, mut bullet) in &mut bullets {
		let step = bullet.velocity * dt;
		let distance = step.length().min(bullet.remaining_range);
		if distance <= 0.0 {
			commands.entity(bullet_entity).despawn();
			continue;
		}
		let Ok(dir) = Dir3::new(bullet.velocity.normalize_or_zero()) else {
			commands.entity(bullet_entity).despawn();
			continue;
		};
		let filter = SpatialQueryFilter::from_mask(HITBOX_LAYER).with_excluded_entities([bullet.shooter]);
		if let Some(hit) = spatial_query.cast_ray(transform.translation, dir, distance, true, &filter) {
			let damaged = apply_damage_per_target(bullet.room_id, vec![hit.entity], bullet.body_damage, bullet.head_damage, &hitbox_query, &mut target_query);
			for target in damaged { commands.entity(target).insert(RecentlyDamaged { timer: DAMAGE_HUD_DURATION }); }
			commands.entity(bullet_entity).despawn();
			continue;
		}
		transform.translation += dir.as_vec3() * distance;
		bullet.remaining_range -= distance;
		if bullet.remaining_range <= 0.0 {
			commands.entity(bullet_entity).despawn();
		}
	}
}

pub(crate) fn update_bullet_visuals(mut commands: Commands, time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut bullets: Query<(Entity, &mut Transform, &mut BulletVisual)>) {
	let dt = time.delta_secs();
	for (entity, mut transform, mut bullet) in &mut bullets {
		let step = bullet.velocity * dt;
		let distance = step.length().min(bullet.remaining_range);
		if distance <= 0.0 {
			commands.entity(entity).despawn();
			continue;
		}
		transform.translation += bullet.velocity.normalize_or_zero() * distance;
		bullet.remaining_range -= distance;
		if bullet.remaining_range <= 0.0 {
			spawn_bullet_spark(&mut commands, &mut meshes, &mut materials, transform.translation);
			commands.entity(entity).despawn();
		}
	}
}

/// Cleans up expired bullet sparks.
pub(crate) fn cleanup_shoot_visuals(mut commands: Commands, time: Res<Time>, mut sparks: Query<(Entity, &mut BulletSpark)>) {
	let dt = time.delta_secs();
	for (e, mut s) in &mut sparks { s.timer -= dt; if s.timer <= 0.0 { commands.entity(e).despawn(); } }
}

fn bullet_spawn_transform(position: Vec3, rotation: Quat, aim_pitch: f32) -> Option<(Vec3, Vec3)> {
	let forward = (rotation * Quat::from_rotation_x(aim_pitch)).mul_vec3(Vec3::NEG_Z).normalize_or_zero();
	Dir3::new(forward).ok()?;
	Some((position + Vec3::Y * 1.55 + forward * 0.45, forward))
}

fn spawn_bullet_visual(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, origin: Vec3, forward: Vec3) {
	let mesh = meshes.add(Capsule3d::new(BULLET_RADIUS, BULLET_VISUAL_LENGTH));
	let material = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.82, 0.35), emissive: (Color::srgb(3.0, 2.0, 0.55)).to_linear() * 1.1, ..default() });
	commands.spawn((
		Name::new("BulletVisual"),
		Mesh3d(mesh),
		MeshMaterial3d(material),
		Transform::from_translation(origin).with_rotation(Quat::from_rotation_arc(Vec3::Y, forward)),
		BulletVisual { velocity: forward * BULLET_SPEED, remaining_range: BULLET_RANGE },
	));
}

fn spawn_bullet_spark(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>, position: Vec3) {
	commands.spawn((
		Name::new("BulletSpark"),
		Mesh3d(meshes.add(Sphere::new(0.12))),
		MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.9, 0.45), emissive: (Color::srgb(3.0, 2.1, 0.6)).to_linear() * 1.3, ..default() })),
		Transform::from_translation(position),
		BulletSpark { timer: BULLET_SPARK_DURATION },
	));
}
