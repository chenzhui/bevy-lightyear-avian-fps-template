use bevy::prelude::*;
use lightyear_replication::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect, Default)]
pub struct ActiveStats {  // Active stats — base value + equipment bonus + buff total
	pub health: f32,      // Current health
	pub melee: f32,       // Current melee damage
	pub shoot: f32,       // Current ranged damage
	pub move_speed: f32,  // Current movement speed
}


pub struct AttributeShaderPlugin;

impl Plugin for AttributeShaderPlugin {
	fn build(&self, app: &mut App) {
		app.register_component::<ActiveStats>();

	}
}


#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct BaseStats {  // Base stats for the character.
	pub health: f32,      // Base health.
	pub melee: f32,       // Base melee damage.
	pub shoot: f32,       // Base ranged damage.
	pub move_speed: f32,  // Base movement speed.
}



// app.register_component::<CharacterAttributes>();

// Attribute container that separates base stats from additive modifiers.
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect, Default)]
pub struct CharacterAttributes {
	pub base: BaseStats,       // Character innate capabilities.
	pub modifiers: BaseStats,  // Equipment or buff modifiers.
}


impl Default for BaseStats {
	fn default() -> Self {
		Self {
			health: 100.0,
			melee: 5.0,
			shoot: 5.0,
			move_speed: 5.0,
		}
	}
}

impl CharacterAttributes {
	/// Computes final active stats from base stats plus modifiers.
	pub fn recalculate(&self) -> ActiveStats {
		ActiveStats {
			health: self.base.health + self.modifiers.health,
			melee: self.base.melee + self.modifiers.melee,
			shoot: self.base.shoot + self.modifiers.shoot,
			move_speed: self.base.move_speed + self.modifiers.move_speed,
		}
	}
}
