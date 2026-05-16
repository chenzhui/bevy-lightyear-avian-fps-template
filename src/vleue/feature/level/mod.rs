
pub(crate) mod types;
pub mod map;
pub mod loading;

use bevy::prelude::*;
use crate::vleue::feature::VleueSide;

pub struct LevelFeaturePlugin {
	pub side: VleueSide,
	pub headless: bool,
}

impl Plugin for LevelFeaturePlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins((
            loading::LoadingPlugin { side: self.side },
            map::MapPlugin { side: self.side, headless: self.headless },
		));
	}
}
