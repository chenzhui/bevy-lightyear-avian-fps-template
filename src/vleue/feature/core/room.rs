use std::collections::HashMap;
use bevy::math::Vec2;
use bevy::prelude::*;
use lightyear::prelude::PeerId;
use lightyear_replication::prelude::{Room, RoomPlugin as LightyearRoomPlugin};
use crate::vleue::feature::core::connection::GameRoomId;
use crate::vleue::feature::core::state::RoomPhaseState;

use crate::vleue::feature::VleueSide;


pub const MATCH_DURATION_SECS: f32 = 40.0 * 60.0;  // Match duration constant (seconds)
pub const FIXED_ROOM_COUNT: u64 = 4; // MVP stage fixed creation of 4 rooms, room numbers 1..=4.

pub struct RoomFeaturePlugin {
	pub side: VleueSide,
}

impl Plugin for RoomFeaturePlugin {
    fn build(&self, app: &mut App) {
        if self.side.is_server() {
            app.add_plugins((LightyearRoomPlugin, RoomStateServerPlugin));
        }
    }
}

#[derive(Debug, Clone)]
pub struct MatchRoomState {  // Single room runtime state
	pub room_id: u64, // Fixed room number
	pub match_id: u64, // Backend real match ID, 0 means currently no bound match.
	pub phase: RoomPhaseState, // Room current phase
	pub elapsed_secs: f32, // This room elapsed time
	pub player_count: u32, // This room current player count
	pub map_loaded: bool, // Whether server has generated map logic for this room.
}

impl Default for MatchRoomState {
	fn default() -> Self {
		Self {
			room_id: 0,
			match_id: 0,
			phase: RoomPhaseState::Waiting,
			elapsed_secs: 0.0,
			player_count: 0,
			map_loaded: false,
		}
	}
}

impl MatchRoomState {
	pub fn new(room_id: u64) -> Self {
		Self {
			room_id,
			..default()
		}
	}

	pub fn bind_match(&mut self, match_id: u64) { // Bind backend match ID to fixed room.
		if self.match_id == 0 {
			self.match_id = match_id;
		}
	}

	pub fn start(&mut self) { // Start this room match
		self.phase = RoomPhaseState::InProgress;
		self.elapsed_secs = 0.0;
		info!("[server] match {} started in room {}", self.match_id, self.room_id);
	}

	pub fn mark_map_loaded(&mut self) { // Mark room map logic ready, start match if players are waiting.
		self.map_loaded = true;
		if self.phase == RoomPhaseState::Waiting && self.player_count > 0 {
			self.start();
		}
	}

	pub fn update(&mut self, delta_secs: f32) { // Update time and check end conditions
		if self.phase != RoomPhaseState::InProgress {
			return;
		}
		self.elapsed_secs += delta_secs;

        // TODO: Re-enable match end conditions when extraction gameplay is finalized.
		// if self.extraction_count >= MAX_EXTRACTIONS || self.elapsed_secs >= MATCH_DURATION_SECS {
		// 	self.phase = RoomPhaseState::Ended;
		// 	println!(
		// 		"[server] match {} in room {} ended, extractions: {}, time: {:.1}min",
		// 		self.match_id,
		// 		self.room_id,
		// 		self.extraction_count,
		// 		self.elapsed_secs / 60.0
		// 	);
		// }
	}

	pub fn can_spawn_players(&self) -> bool {
		matches!(self.phase, RoomPhaseState::InProgress)
	}

	pub fn is_ended(&self) -> bool {
		self.phase == RoomPhaseState::Ended
	}
}


#[derive(Resource, Default)]
pub struct RoomManager { /// Room entity management
	pub rooms: HashMap<u64, Entity>, // room_id -> room_entity
}


#[derive(Resource, Default, Clone)]
pub struct PendingPlayers { // Players waiting to join match
	pub players: Vec<(Entity, PeerId, Vec2, u64, u64)>, // (connection_entity, client_id, spawn_pos, room_id, match_id)
}

/// All room states current server instance carries
#[derive(Resource, Default, Debug)]
pub struct MatchRoomStates {
	pub rooms: HashMap<u64, MatchRoomState>, // room_id -> runtime state
}

impl MatchRoomStates {
	pub fn ensure_room(&mut self, room_id: u64) -> &mut MatchRoomState {
		self.rooms.entry(room_id).or_insert_with(|| MatchRoomState::new(room_id))
	}

	pub fn register_player(&mut self, room_id: u64, match_id: u64) {
		let room_state = self.ensure_room(room_id);
		room_state.bind_match(match_id);
		room_state.player_count += 1;
		if room_state.phase == RoomPhaseState::Waiting && room_state.map_loaded {
			room_state.start();
		}
	}

	pub fn mark_map_loaded(&mut self, room_id: u64) {
		self.ensure_room(room_id).mark_map_loaded();
	}

	pub fn can_spawn_players(&self, room_id: u64) -> bool {
		self.rooms.get(&room_id).is_some_and(MatchRoomState::can_spawn_players)
	}

	pub fn elapsed_secs(&self, room_id: u64) -> Option<f32> {
		self.rooms.get(&room_id).map(|room| room.elapsed_secs)
	}

	pub fn update_all(&mut self, delta_secs: f32) {
		for room_state in self.rooms.values_mut() {
			room_state.update(delta_secs);
		}
	}
}

/// Server room state plugin
pub struct RoomStateServerPlugin;

impl Plugin for RoomStateServerPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<RoomManager>();
		app.init_resource::<PendingPlayers>();
		app.init_resource::<MatchRoomStates>();
		app.add_systems(Startup, setup_fixed_rooms);
	}
}

fn setup_fixed_rooms(mut commands: Commands, mut room_manager: ResMut<RoomManager>, mut room_states: ResMut<MatchRoomStates>) { // Create fixed rooms on server startup, MVP doesn't do on-demand creation yet.
	for room_id in 1..=FIXED_ROOM_COUNT {
		if room_manager.rooms.contains_key(&room_id) {
			continue;
		}
		let room_entity = commands.spawn((
			Room::default(),
			GameRoomId(room_id),
			Name::new(format!("Room_{}", room_id)),
		)).id();
		room_manager.rooms.insert(room_id, room_entity);
		room_states.ensure_room(room_id);
		info!("[server] fixed room {} created entity={:?}", room_id, room_entity);
	}
}

