use bevy::app::{App, Plugin, Startup, Update};
use bevy::camera::ClearColor;
use bevy::prelude::{Commands, Query, Res, Resource, Window};
use bevy::prelude::*;




#[derive(Resource)]
pub(crate) struct GameName( pub String);

pub fn set_window_title(mut window: Query<&mut Window>, game_name: Res<GameName>) {
	let mut window = window.single_mut().unwrap();
	window.title = format!("Lightyear Example: {}", game_name.0);
}




use crate::vleue::feature::VleueSide;

pub struct VleueWindowPlugin {
	pub side: VleueSide,
	pub name: String,
    pub server_room: Option<u64>,
}

impl Plugin for VleueWindowPlugin {
	fn build(&self, app: &mut App) {
		match self.side {
			VleueSide::Client => {
				app.add_plugins(ClientWindowPlugin { name: self.name.clone() });
			}
			VleueSide::Server => {
				app.add_plugins(ServerWindowInitPlugin { name: self.name.clone(), room: self.server_room });
			}
		}
	}
}

//region shader

//endregion shader

//region server
pub struct ServerWindowInitPlugin {
    pub name: String,
    pub room: Option<u64>,
}

impl ServerWindowInitPlugin {
    pub fn new(name: String, room: Option<u64>) -> Self {
        Self { name, room }
    }
}

impl Plugin for ServerWindowInitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GameName(self.name.clone()));
        app.insert_resource(ClearColor::default());
        // if let Some(room) = self.room {
        //     app.insert_resource(ServerDebugRoomId(room));
        // }

    }
}

// #[derive(Resource, Debug, Clone, Copy)]
// pub struct ServerDebugRoomId(pub u64); // Server window currently observing room number, not involved in network sync.



//endregion server

//region client

pub struct ClientWindowPlugin {
	pub name: String,
}

impl ClientWindowPlugin {
	pub fn new(name: String) -> Self {
		Self { name }
	}
}



impl Plugin for ClientWindowPlugin {
	fn build(&self, app: &mut App) {
		debug!("client plugin build");
		app.insert_resource(GameName(self.name.clone()));
		app.insert_resource(ClearColor::default());
		app.add_systems(Startup, set_window_title);
	}
}

//endregion client


