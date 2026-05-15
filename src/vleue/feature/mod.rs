pub mod core;  // Physics and entity representation: player, NPC, basic physics.
pub mod lobby;
pub mod character;
// If your game suddenly becomes a pure social lobby (no enemies, no combat, only chat and wandering), which code must remain? Which can be deleted? Must remain -> belongs to character (inherent to the character itself). Can delete -> belongs to combat (damage or adversarial interactions).
pub mod level;
pub mod physics;

// Gameplay layer: combat, pickup, lobby, extraction and other systems.

use bevy::prelude::*;

pub struct FeaturePlugin {
	pub side: VleueSide,
	pub headless: bool,
}

impl Plugin for FeaturePlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins((
			physics::VleuePhysicsPlugin, // Physics base loaded first
			core::CoreFeaturePlugin { side: self.side },
			character::CharacterPlugin { side: self.side },
			#[cfg(feature = "combat")]
			combat::CombatPlugin { side: self.side },
			lobby::LobbyPlugin { side: self.side },
			level::LevelFeaturePlugin { side: self.side, headless: self.headless },
		));
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VleueSide {
	Client,
	Server,
}

impl VleueSide {
	pub fn is_client(self) -> bool {
		matches!(self, Self::Client)
	}

	pub fn is_server(self) -> bool {
		matches!(self, Self::Server)
	}
}
