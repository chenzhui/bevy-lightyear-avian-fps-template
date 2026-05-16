pub mod window; // Application window, port and basic UI entry.
pub use window::{ClientWindowPlugin, ServerWindowInitPlugin, VleueWindowPlugin};
pub mod state; // Client state machine.
pub mod connection; // Player connection, room joining and character spawning.
pub mod net; // HTTP client and common async request utilities.
pub mod room; // Server room state and match room runtime data.
pub mod settings; // Local config and keybind settings.
pub mod i18n; // Internationalization and multilingual support.
pub mod health;
pub mod server_debug;
// Battle server health check HTTP endpoint.

use bevy::app::{App, Plugin};
use crate::vleue::feature::VleueSide;

pub struct CoreFeaturePlugin {
	pub side: VleueSide, // core trunk: unified network, room, state and other underlying capabilities.
}

impl Plugin for CoreFeaturePlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(
			(net::NetPlugin { side: self.side },
			 connection::ConnectionPlugin { side: self.side },
			 state::StateFeaturePlugin { side: self.side },
			 room::RoomFeaturePlugin { side: self.side },
			 settings::SettingsPlugin { side: self.side },
			 i18n::I18nPlugin { side: self.side },
			 health::GameServerHealthPlugin { side: self.side }
			)
		);
	}
}
