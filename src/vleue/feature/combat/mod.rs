use bevy::prelude::*;
use lightyear_replication::prelude::AppComponentExt;
use crate::vleue::feature::VleueSide;

pub mod types; // Basic data structures (damage types, events, resistances, health, combat state, etc.)
pub mod death;
pub mod attribute;
pub mod damage; // Server-authoritative combat logic.
pub mod projectile; // Client-side shooting visuals.
pub mod hitbox; // Melee/area hit detection systems.
// Downed, bleeding, and revive systems built on shared life state.

// Re-export common types
pub use death::{DeathPlugin, LifeState, LifeStatus};
pub use types::{AmmoState, Armor, CombatLoadout, CombatState, DamageEvent, DamageType, Health, MedicalState, RecentlyDamaged, Resistances, SkillSlot, SkillState, SquadMemberInfo, SquadState};

pub struct CombatPlugin { pub side: VleueSide }

impl Plugin for CombatPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(CombatShaderPlugin { side: self.side });
		match self.side {
			VleueSide::Client => app.add_plugins(CombatClientPlugin),
			VleueSide::Server => app.add_plugins(CombatServerPlugin),
		};
	}
}

//region shader
struct CombatShaderPlugin {
	side: VleueSide,
}

impl Plugin for CombatShaderPlugin {
	fn build(&self, app: &mut App) {
		app.register_component::<Health>();
		app.register_component::<CombatLoadout>();
		app.register_component::<RecentlyDamaged>();
		app.register_component::<Armor>();
		app.register_component::<AmmoState>();
		app.register_component::<SkillState>();
		app.register_component::<MedicalState>();
		app.add_plugins(attribute::AttributeShaderPlugin);
		app.add_plugins(DeathPlugin { side: self.side });
		app.add_systems(Update, (damage::tick_combat_state, damage::tick_recently_damaged,));
	}
}
//endregion shader

//region server

struct CombatServerPlugin;

impl Plugin for CombatServerPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(FixedUpdate, (damage::handle_player_combat, projectile::spawn_server_bullets, projectile::update_server_bullets,));
	}
}

//endregion server

//region client

struct CombatClientPlugin;

impl Plugin for CombatClientPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Update, (projectile::handle_client_shoot_visuals, projectile::update_bullet_visuals, projectile::cleanup_shoot_visuals,));
	}
}

//endregion client
