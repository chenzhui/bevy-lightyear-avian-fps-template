use bevy::prelude::*;
use crate::vleue::feature::VleueSide;

pub mod matching;
pub mod ui;

pub struct LobbyPlugin {
	pub side: VleueSide,
}

impl Plugin for LobbyPlugin {
	fn build(&self, app: &mut App) {
		if self.side.is_client() {
			app.add_plugins(matching::MatchClientPlugin);
			app.add_plugins(ui::LobbyUiClientPlugin);
		}
	}
}
