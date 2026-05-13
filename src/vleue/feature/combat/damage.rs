use avian3d::physics_transform::{Position, Rotation};
use avian3d::prelude::{Collider, SpatialQuery, SpatialQueryFilter};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use crate::vleue::feature::character::animate::CharacterAnim;
use crate::vleue::feature::character::input::CharacterAttackIntent;
use crate::vleue::feature::character::movement::{CharacterAimPitch, CharacterHitboxKind, CharacterMarker, HITBOX_LAYER};
use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::combat::{LifeState, LifeStatus};
use crate::vleue::feature::combat::types::{CombatLoadout, CombatState, Health, RecentlyDamaged};
use crate::vleue::feature::core::connection::GameRoomId;

//region Constants ────────────────────────────────────────────────
const MELEE_COOLDOWN: f32 = 0.55;/// Melee attack cooldown in seconds.
const MELEE_HIT_DELAY: f32 = 0.12;/// Delay between swing start and damage application.
const MELEE_ANIM_LOCK: f32 = 0.42;/// Melee animation lock duration in seconds.
const MELEE_RANGE: f32 = 1.35;/// Melee attack range.
const MELEE_RADIUS: f32 = 0.85;/// Melee spherical hit radius.
pub const SHOOT_COOLDOWN: f32 = 0.22;/// Ranged attack cooldown in seconds.
pub const SHOOT_ANIM_LOCK: f32 = 0.18;/// Ranged attack animation lock duration in seconds.
pub const DAMAGE_HUD_DURATION: f32 = 2.0;/// Damaged HUD visibility duration in seconds.
//endregion

//region System Entry Points ────────────────────────────────────────────

/// Ticks combat state timers for cooldowns, hit delays, and animation locks.
pub fn tick_combat_state(time: Res<Time>, mut query: Query<&mut CombatState>) {
	let dt = time.delta_secs();
	for mut state in &mut query {
		state.melee_cooldown = (state.melee_cooldown - dt).max(0.0);
		state.shoot_cooldown = (state.shoot_cooldown - dt).max(0.0);
		if state.melee_damage_pending { state.melee_hit_timer = (state.melee_hit_timer - dt).max(0.0); }
		if let Some((anim, t)) = state.animation_lock {
			let next = (t - dt).max(0.0);
			state.animation_lock = (next > 0.0).then_some((anim, next));
		}
	}
}

/// Ticks damaged-highlight timers and removes `RecentlyDamaged` when they expire.
pub fn tick_recently_damaged(time: Res<Time>, mut commands: Commands, mut query: Query<(Entity, &mut RecentlyDamaged)>) {
	for (entity, mut damaged) in &mut query {
		damaged.timer -= time.delta_secs();
		if damaged.timer <= 0.0 { commands.entity(entity).remove::<RecentlyDamaged>(); }
	}
}

/// Server-side player combat input handling for melee and ranged attacks.
pub fn handle_player_combat(mut commands: Commands, spatial_query: SpatialQuery, hitbox_query: Query<(&CharacterHitboxKind, &ChildOf)>, mut target_query: Query<(&mut Health, &mut LifeState, &GameRoomId)>,
	mut attack_events: MessageReader<CharacterAttackIntent>, mut attackers: Query<(Entity, &GameRoomId, &Position, &Rotation, &CharacterAimPitch, &CombatLoadout, &mut CombatState), (With<CharacterMarker>, With<VleuePlayer>)>,
) {
	let attack_entities: Vec<Entity> = attack_events.read().map(|event| event.entity).collect();
	let mut all_damaged: Vec<Entity> = Vec::new();
	for (entity, room_id, position, rotation, _aim_pitch, loadout, mut state) in &mut attackers {
		let Ok((_, life, _)) = target_query.get(entity) else { continue; };
		if life.status != LifeStatus::Alive { continue; }
		// Melee.
		if attack_entities.contains(&entity) && state.melee_cooldown <= 0.0 {
			state.melee_cooldown = MELEE_COOLDOWN;
			state.melee_hit_timer = MELEE_HIT_DELAY;
			state.melee_damage_pending = true;
			state.animation_lock = Some((CharacterAnim::Attack, MELEE_ANIM_LOCK));
		}
		if state.melee_damage_pending && state.melee_hit_timer <= 0.0 {
			let damaged = resolve_melee(entity, room_id.0, position.0, rotation.0, loadout, &spatial_query, &hitbox_query, &mut target_query);
			all_damaged.extend(damaged);
			state.melee_damage_pending = false;
		}
	}
	for target in all_damaged { commands.entity(target).insert(RecentlyDamaged { timer: DAMAGE_HUD_DURATION }); }
}

//endregion

//region Damage Resolution ───────────────────────────────────────────

/// Resolves a melee attack by checking a spherical hit area in front of the player.
fn resolve_melee(attacker: Entity, room_id: u64, pos: Vec3, rot: Quat, loadout: &CombatLoadout, spatial_query: &SpatialQuery, hitbox_query: &Query<(&CharacterHitboxKind, &ChildOf)>, target_query: &mut Query<(&mut Health, &mut LifeState, &GameRoomId)>) -> Vec<Entity> {
	let origin = pos + Vec3::Y * 1.0;
	cast_melee_and_apply(attacker, room_id, origin, rot, MELEE_RANGE, MELEE_RADIUS, loadout.melee_body_damage, loadout.melee_head_damage, spatial_query, hitbox_query, target_query)
}

/// Casts the melee shape and applies per-hitbox damage.
pub fn cast_melee_and_apply(attacker: Entity, room_id: u64, origin: Vec3, rot: Quat, range: f32, radius: f32, body_dmg: f32, head_dmg: f32, spatial_query: &SpatialQuery, hitbox_query: &Query<(&CharacterHitboxKind, &ChildOf)>, target_query: &mut Query<(&mut Health, &mut LifeState, &GameRoomId)>) -> Vec<Entity> {
	let filter = SpatialQueryFilter::from_mask(HITBOX_LAYER).with_excluded_entities([attacker]);
	let center = origin + rot.mul_vec3(Vec3::new(0.0, 0.0, -range));
	let intersections = spatial_query.shape_intersections(&Collider::sphere(radius), center, Quat::IDENTITY, &filter);
	apply_damage_per_target(room_id, intersections, body_dmg, head_dmg, hitbox_query, target_query)
}

/// Applies the best hitbox damage per target and resolves downed/dead transitions.
///
/// Returns all entities that actually took damage so callers can mark `RecentlyDamaged`.
pub fn apply_damage_per_target(room_id: u64, hits: Vec<Entity>, body_dmg: f32, head_dmg: f32, hitbox_query: &Query<(&CharacterHitboxKind, &ChildOf)>, target_query: &mut Query<(&mut Health, &mut LifeState, &GameRoomId)>) -> Vec<Entity> {
	// Deduplicate per target and keep the best hitbox damage.
	let mut best: Vec<(Entity, f32)> = Vec::new();
	for hit in hits {
		let Ok((kind, parent)) = hitbox_query.get(hit) else { continue; };
		let target = parent.parent();
		let dmg = match kind { CharacterHitboxKind::Head => head_dmg, CharacterHitboxKind::Body => body_dmg };
		if let Some((_, stored)) = best.iter_mut().find(|(t, _)| *t == target) { *stored = stored.max(dmg); }
		else { best.push((target, dmg)); }
	}
	// Apply damage.
	let mut damaged = Vec::new();
	for (target, dmg) in best {
		let Ok((mut health, mut life, tr)) = target_query.get_mut(target) else { continue; };
		if tr.0 != room_id || life.status == LifeStatus::Dead { continue; }
		match life.status {
			LifeStatus::Alive => {
				health.current -= dmg;
				if health.current <= 0.0 { health.current = 500.0; life.status = LifeStatus::Downed; life.downed_timer = 0.0; }
			}
			LifeStatus::Downed => {
				health.current -= dmg;
				if health.current <= 0.0 || dmg > 0.0 { health.current = 0.0; life.status = LifeStatus::Dead; }
			}
			LifeStatus::Dead => {}
		}
		if dmg > 0.0 { damaged.push(target); }
	}
	damaged
}

//endregion
