//! Lobby server HTTP client
//!
//! Uses entity-based Task + serde_json::Value to adapt Bevy's ECS architecture, efficient and non-blocking

use bevy::prelude::*;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::tasks::IoTaskPool;
use serde_json::{json, Value};
use crate::vleue::feature::core::connection::{connect_to_match_server, MatchData};
use crate::vleue::feature::core::net::{BackendClient, HttpResponseEvent, HttpTask, clear_http_tasks, has_pending_http_action, json_bool, json_i32, json_string, json_u32, json_u64, spawn_http_task};
use crate::vleue::feature::core::state::{AppClientState, InGameConnectionState, LobbyState, PendingLobbyState, UserId};

pub const ACTION_JOIN_MATCH: &str = "JoinMatch";
pub const ACTION_QUERY_STATUS: &str = "QueryStatus";
pub const ACTION_CANCEL_MATCH: &str = "CancelMatch";
pub const MATCH_STATUS_POLL_INTERVAL: f32 = 2.0;

#[derive(Message, Clone, Copy, Debug)]
pub struct QueueCancelledEvent; // Instant event emitted after backend confirms cancellation, state returns to Idle.

#[derive(Default, Resource)]
pub struct QueueUiState {
	pub last_query_time: f32,
	pub current_queue_size: i32,
}

pub struct MatchClientPlugin;

impl Plugin for MatchClientPlugin {
	fn build(&self, app: &mut App) {
		let in_lobby = in_state(AppClientState::Lobby);
		let in_game = in_state(AppClientState::InGame);

		app.insert_resource(QueueUiState::default());
		app.add_message::<QueueCancelledEvent>();

		app.add_systems(OnEnter(AppClientState::Lobby), setup_lobby_client);
		app.add_systems(Update, apply_pending_lobby_state.run_if(in_lobby.clone()));
		app.add_systems(Update, handle_queue_cancelled.run_if(in_lobby.clone()));
		app.add_systems(Update, poll_match_status.run_if(in_lobby.clone().and(in_state(LobbyState::Queuing))));
		app.add_systems(Update, poll_match_status.run_if(in_game.clone().and(in_state(InGameConnectionState::Reconnecting))));
		app.add_systems(Update, handle_http_responses.run_if(in_lobby.clone()));
		app.add_systems(Update, handle_http_responses.run_if(in_game.clone().and(in_state(InGameConnectionState::Reconnecting))));
	}
}

pub fn setup_lobby_client(mut commands: Commands) {
	commands.insert_resource(QueueUiState::default());
	commands.insert_resource(MatchData::default());
	info!("🌐 Lobby HTTP client initialized");
}

fn apply_pending_lobby_state(mut pending_state: ResMut<PendingLobbyState>, mut next_state: ResMut<NextState<LobbyState>>) {
	let Some(state) = pending_state.0.take() else { return; };
	next_state.set(state);
}

pub fn start_matchmaking(commands: &mut Commands<'_, '_>, client: &BackendClient, user_id: u64, rating: i32, match_data: &mut MatchData, queue_state: &mut QueueUiState, http_tasks: &Query<(Entity, &HttpTask)>, next_state: &mut NextState<LobbyState>) {
	clear_http_tasks(commands, http_tasks, &[ACTION_JOIN_MATCH, ACTION_QUERY_STATUS, ACTION_CANCEL_MATCH]);
	match_data.clear();
	queue_state.current_queue_size = 0;
	request_join_match(commands, client, user_id, rating);
	next_state.set(LobbyState::Queuing);
}

pub fn cancel_matchmaking(commands: &mut Commands<'_, '_>, client: &BackendClient, user_id: u64, http_tasks: &Query<(Entity, &HttpTask)>) {
	if http_tasks.iter().any(|(_, task)| task.action == ACTION_CANCEL_MATCH) {
		return;
	}
	request_cancel_match(commands, client, user_id);
}

fn handle_queue_cancelled(mut events: MessageReader<QueueCancelledEvent>, mut queue_state: ResMut<QueueUiState>, mut match_data: ResMut<MatchData>) {
	for _ in events.read() {
		queue_state.last_query_time = 0.0;
		queue_state.current_queue_size = 0;
		match_data.clear();
	}
}

pub fn request_join_match(commands: &mut Commands<'_, '_>, client: &BackendClient, user_id: u64, rating: i32) {
	let http = client.http.clone(); 
	let base_url = client.base_url.clone();
	let payload = json!({
		"userId": user_id,
		"rating": rating,
	});
	let task = IoTaskPool::get().spawn(async move {
		let url = format!("{}/api/match/join", base_url);
		let response = http.post(&url).json(&payload).send().map_err(|e| format!("Request failed: {}", e))?;
		response.json::<Value>().map_err(|e| format!("Response parsing failed: {}", e))
	});
	spawn_http_task(commands, ACTION_JOIN_MATCH, task);
}

pub fn request_cancel_match(commands: &mut Commands<'_, '_>, client: &BackendClient, user_id: u64) {
	let base_url = client.base_url.clone();
	let http = client.http.clone();
	let task = IoTaskPool::get().spawn(async move {
		let url = format!("{}/api/match/cancel/{}", base_url, user_id);
		let response = http.delete(&url).send().map_err(|e| format!("Request failed: {}", e))?;
		response.json::<Value>().map_err(|e| format!("Parsing failed: {}", e))
	});
	spawn_http_task(commands, ACTION_CANCEL_MATCH, task);
}

pub fn request_query_status(commands: &mut Commands<'_, '_>, client: &BackendClient, user_id: u64) {
	let http = client.http.clone();
	let base_url = client.base_url.clone();
	let task = IoTaskPool::get().spawn(async move {
		let url = format!("{}/api/match/status/{}", base_url, user_id);
		let response = http.get(&url).send().map_err(|e| format!("Request failed: {}", e))?;
		response.json::<Value>().map_err(|e| format!("Parsing failed: {}", e))
	});
	spawn_http_task(commands, ACTION_QUERY_STATUS, task);
}

pub fn poll_match_status(mut commands: Commands, client: Res<BackendClient>, mut queue_state: ResMut<QueueUiState>, time: Res<Time>, user_id: Res<UserId>, tasks: Query<&HttpTask>) {
	if has_pending_http_action(&tasks, ACTION_QUERY_STATUS) {
		return;
	}
	queue_state.last_query_time += time.delta_secs();
	if queue_state.last_query_time < MATCH_STATUS_POLL_INTERVAL {
		return;
	}
	queue_state.last_query_time = 0.0;
	request_query_status(&mut commands, &client, user_id.0);
}

pub fn handle_http_responses(mut commands: Commands, mut events: MessageReader<HttpResponseEvent>, app_state: Res<State<AppClientState>>, current_lobby_state: Option<Res<State<LobbyState>>>, current_ingame_state: Option<Res<State<InGameConnectionState>>>, mut queue_state: ResMut<QueueUiState>, mut match_data: ResMut<MatchData>, user_id: Res<UserId>, mut cancel_writer: MessageWriter<QueueCancelledEvent>, mut next_app_state: ResMut<NextState<AppClientState>>, mut next_lobby_state: ResMut<NextState<LobbyState>>, mut next_ingame_state: ResMut<NextState<InGameConnectionState>>) {
	for event in events.read() {
		match event.action {
			ACTION_JOIN_MATCH => {
				if current_lobby_state.as_deref().map(State::get) != Some(&LobbyState::Queuing) {
					continue;
				}
				queue_state.current_queue_size = json_i32(&event.data, "queueSize");
			}
			ACTION_QUERY_STATUS => {
				let is_lobby_queue = app_state.get() == &AppClientState::Lobby && current_lobby_state.as_deref().map(State::get) == Some(&LobbyState::Queuing);
				let is_ingame_reconnect = app_state.get() == &AppClientState::InGame && current_ingame_state.as_deref().map(State::get) == Some(&InGameConnectionState::Reconnecting);
				if !is_lobby_queue && !is_ingame_reconnect {
					continue;
				}

				if apply_match_status(&event.data, &mut queue_state, &mut match_data) {
					let netcode_client_id = match_data.netcode_client_id.unwrap_or(user_id.0);
					if match_data.netcode_client_id.is_none() {
						warn!("Match success response missing netcode client id, falling back to user id: {}", event.data);
					}
					connect_to_match_server(&mut commands, &mut match_data, netcode_client_id);
					next_ingame_state.set(InGameConnectionState::Connecting);
					if is_lobby_queue {
						next_app_state.set(AppClientState::InGame);
					}
				}
			}
			ACTION_CANCEL_MATCH => {
				if app_state.get() == &AppClientState::Lobby && current_lobby_state.as_deref().map(State::get) == Some(&LobbyState::Queuing) {
					cancel_writer.write(QueueCancelledEvent);
					next_lobby_state.set(LobbyState::Idle);
				}
			}
			_ => warn!("Received unknown API response: {}", event.action),
		}
	}
}

fn apply_match_status(data: &Value, queue_state: &mut QueueUiState, match_data: &mut MatchData) -> bool {
    debug!("{}", data);
	queue_state.current_queue_size = json_i32(data, "queueSize");
	if !json_bool(data, "matched") {
		return false;
	}
	let (Some(match_id), Some(room_id), Some(server_host), Some(server_port), Some(entry_token)) = (
		json_u64(data, "matchId"),
		json_u64(data, "roomId"),
		json_string(data, "serverHost"),
		json_u32(data, "serverPort"),
		json_string(data, "entryToken"),
	) else {
		warn!("Match success response missing battle server connection info: {}", data);
		return false;
	};
	match_data.match_id = Some(match_id);
	match_data.room_id = Some(room_id);
	match_data.server_host = Some(server_host);
	match_data.server_port = Some(server_port);
	match_data.entry_token = Some(entry_token);
	match_data.netcode_client_id = json_u64(data, "netcodeClientId");
	true
}

// pub fn finish_match_loading(mut next_state: ResMut<NextState<AppClientState>>, player_query: Query<(), (With<crate::vleue::feature::character::VleuePlayer>, With<crate::vleue::feature::character::movement::CharacterMarker>, With<lightyear::prelude::Predicted>, With<lightyear::prelude::Controlled>)>) {
// 	if player_query.is_empty() { return; }
// 	next_state.set(AppClientState::InGame);
// }
