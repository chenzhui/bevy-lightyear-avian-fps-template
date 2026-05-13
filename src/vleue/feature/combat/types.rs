use bevy::prelude::*;
use serde::{Deserialize, Serialize};

//region Damage types & events ────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Reflect)]
pub enum DamageType { Physical, Fire, Frost, Poison, Lightning, Arcane } // Damage type enum (used with resistance system for damage reduction)

impl Default for DamageType { fn default() -> Self { Self::Physical } }

#[derive(Event, Clone, Debug)]
pub struct DamageEvent {
	pub attacker: Entity,        // Attacker entity (may be projectile, player, environment)
	pub caster: Option<Entity>,  // Actual caster (player when projectile damage)
	pub target: Entity,          // Target entity
	pub amount: f32,             // Base damage value
	pub dmg_type: DamageType,    // Damage type
	pub is_crit: bool,           // Is critical hit
	pub headshot: bool,          // Is headshot
}

//endregion

//region Resistances & status ───────────────────────────────────────

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
pub struct Resistances {
	pub physical: f32, pub fire: f32, pub frost: f32,
	pub poison: f32, pub lightning: f32, pub arcane: f32,
}

impl Resistances {
	pub fn mitigation(&self, dmg_type: DamageType) -> f32 { // Get damage reduction ratio by damage type, max 95%
		match dmg_type {
			DamageType::Physical => self.physical, DamageType::Fire => self.fire,
			DamageType::Frost => self.frost, DamageType::Poison => self.poison,
			DamageType::Lightning => self.lightning, DamageType::Arcane => self.arcane,
		}.clamp(0.0, 0.95)
	}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Reflect, Serialize, Deserialize)]
pub enum StatusType { Burning, Frozen, Poisoned, Stunned, Slowed } // Status effect type (applied after projectile/attack hit)

#[derive(Component, Clone, Debug)]
pub struct DamageOnHit { pub amount: f32, pub dmg_type: DamageType, pub caster: Option<Entity> } // Damage on hit (attached to projectile or melee attack box)

#[derive(Component, Clone, Debug)]
pub struct ApplyStatusOnHit { pub status: StatusType, pub duration: f32, pub magnitude: f32 } // Apply status effect on hit (attached to projectile)

//endregion

//region Health ────────────────────────────────────────────

#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct Health { pub current: f32, pub max: f32 } // Character current and max health

impl Health {
	pub fn new(max: f32) -> Self { Self { current: max, max } } // Create full health state
	pub fn set_max_preserving_ratio(&mut self, new_max: f32) { // Adjust max health preserving current ratio (called when equipment bonus)
		let clamped = new_max.max(1.0);
		let ratio = if self.max > 0.0 { (self.current / self.max).clamp(0.0, 1.0) } else { 1.0 };
		self.max = clamped;
		self.current = (self.max * ratio).clamp(0.0, self.max);
	}
}

pub fn calculate_final_damage(base: f32, dmg_type: DamageType, resistances: Option<&Resistances>, is_crit: bool, headshot: bool) -> f32 { // Calculate final damage (combining resistance, crit, headshot bonus)
	let mit = resistances.map(|r| r.mitigation(dmg_type)).unwrap_or(0.0);
	let mut dmg = base * (1.0 - mit);
	if is_crit { dmg *= 1.5; }
	if headshot { dmg *= 2.0; }
	dmg
}

//endregion

//region Combat loadout & state ──────────────────────────────────

#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
pub struct CombatLoadout {
	pub melee_body_damage: f32,
	pub melee_head_damage: f32,
	pub shoot_body_damage: f32,
	pub shoot_head_damage: f32,
}

impl CombatLoadout {
	pub fn player_default() -> Self { // Player default combat attributes
		Self { melee_body_damage: 25.0, melee_head_damage: 40.0, shoot_body_damage: 20.0, shoot_head_damage: 45.0 }
	}
	pub fn enemy_default() -> Self { // Generic melee enemy default combat attributes (no ranged attack)
		Self { melee_body_damage: 18.0, melee_head_damage: 30.0, shoot_body_damage: 0.0, shoot_head_damage: 0.0 }
	}
	pub fn tiger_default() -> Self { Self::enemy_default() } // Compatible with old project code, can be removed in closed-source side
}

#[derive(Component, Clone, Debug, PartialEq, Default)]
pub struct CombatState {
	pub melee_cooldown: f32,         // Melee attack cooldown (seconds)
	pub shoot_cooldown: f32,         // Ranged attack cooldown (seconds)
	pub skill_q_cooldown: f32,       // Q skill cooldown (seconds)
	pub melee_hit_timer: f32,        // Melee hit delay timer
	pub melee_damage_pending: bool,  // Has pending melee damage frame
	pub animation_lock: Option<(crate::vleue::feature::character::animate::CharacterAnim, f32)>, // (Animation type, remaining lock time)
}

impl CombatState {
	pub fn locked_animation(&self) -> Option<crate::vleue::feature::character::animate::CharacterAnim> { // Get currently locked animation (if has remaining time)
		self.animation_lock.and_then(|(anim, t)| (t > 0.0).then_some(anim))
	}
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct RecentlyDamaged { pub timer: f32 } // Hit highlight marker (used for client enemy health bar display)

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct Armor { pub current: f32, pub max: f32 } // Armor component (similar to Apex shield)

impl Armor {
	pub fn new(max: f32) -> Self { Self { current: max, max } }
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct AmmoState {
	pub magazine: u32,
	pub reserve: u32,
	pub weapon_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct SquadMemberInfo {
	pub name: String,
	pub health_current: f32,
	pub health_max: f32,
	pub armor_current: f32,
	pub armor_max: f32,
	pub status: crate::vleue::feature::combat::death::LifeStatus,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct SquadState { pub members: Vec<SquadMemberInfo> } // Squad state (contains all teammate info, client Resource)

#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct SkillSlot {
	pub name: String,
	pub cooldown: f32,      // Remaining cooldown (seconds)
	pub max_cooldown: f32,  // Total cooldown (seconds)
}

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct SkillState { pub slots: Vec<SkillSlot> } // Skill state (mounted on player entity)

#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub struct MedicalState {
	pub medkits: u32,
	pub bandages: u32,
	pub adrenaline: u32,
}

//endregion
