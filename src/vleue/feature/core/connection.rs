use bevy::app::{App, Plugin};
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use futures_lite::future;
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

use lightyear::connection::client_of::ClientOf;
use lightyear::prelude::{AppChannelExt, AppMessageExt, ChannelMode, ChannelSettings, Client as NetworkClient, Connect, Connected, Connecting, Disconnect, Disconnected, Linked, LinkOf, MessageReceiver, MessageSender, NetworkDirection, NetworkTarget, PeerId, ReliableSettings, RemoteId, Unlinked};
use lightyear_replication::prelude::{AppComponentExt, ControlledBy, DisableReplicateHierarchy, InterpolationTarget, Lifetime, PredictionTarget, Replicate, ReplicationSender, RoomEvent, RoomTarget, SendUpdatesMode};
use crate::vleue::cli_connection::{shared_settings, ClientConnection, ClientTransports, SEND_INTERVAL};
use crate::vleue::feature::character::{VleueClientId, VleuePlayer};
use crate::vleue::feature::VleueSide;
use crate::vleue::feature::core::net::BackendClient;
use crate::vleue::feature::core::room::{FIXED_ROOM_COUNT, MatchRoomStates, PendingPlayers, RoomManager};
use crate::vleue::feature::core::state::{AppClientState, InGameConnectionState, InGameState, UserId};
use crate::vleue::util::env::env_string;

#[derive(Resource, Clone, Debug)]
pub struct LobbyBackendConfig {
	pub base_url: String, // Lobby backend HTTP address
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct GameRoomId(pub u64); // Room number, used for multi-room isolation.

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, Reflect)]
pub struct MatchId(pub u64); // Backend real match ID, used for settlement and battle records.

#[derive(Resource, Default)]
pub struct MatchData { // Client current match connection data, filled by lobby match result, read when connecting to battle server.
	pub match_id: Option<u64>, // Backend returned match ID.
	pub room_id: Option<u64>, // Backend assigned fixed room number.
	pub server_host: Option<String>, // Battle server address.
	pub server_port: Option<u32>, // Battle server port.
	pub entry_token: Option<String>, // Lobby issued entry token, sent to battle server for validation after connection.
	pub netcode_client_id: Option<u64>, // Lobby assigned netcode client id, used to build ConnectToken.
	pub connection_started: bool, // Whether battle server connection has been created, avoids duplicate spawn of connection entity.
	pub connection_entity: Option<Entity>, // Current battle server connection entity, for reset and cleanup on failure.
}

impl MatchData {
	pub fn clear(&mut self) { // Clear current match and connection data, called when re-queuing or returning to lobby.
		self.match_id = None;
		self.room_id = None;
		self.server_host = None;
		self.server_port = None;
		self.entry_token = None;
		self.netcode_client_id = None;
		self.connection_started = false;
		self.connection_entity = None;
	}
}

#[derive(Component, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component)]
pub struct PendingMatchServerConnection { // Marker that client is connecting to battle server, records if connection and room validation have been initiated.
	connect_requested: bool, // Whether Lightyear Connect has been triggered, avoids per-frame duplicate trigger.
	join_request_sent: bool, // Whether JoinMatchRequest has been sent, avoids duplicate room validation.
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct JoinMatchRequest {  // Match join request (client -> server)
	pub match_id: u64, // Client received match ID
	pub room_id: u64, // Client received fixed room number
	pub player_id: u64, // Client self-reported player ID
	pub entry_token: String, // Lobby issued entry ticket
}

pub struct JoinMatchChannel; // Used to put JoinMatchRequest in separate reliable channel, avoid mixing with other messages

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct MatchStartNotification { // Match start notification (server -> client)
	pub match_id: u64, // Server confirmed match ID
	pub room_id: u64, // Server confirmed fixed room number
	pub start_time: f64, // Match logic start time
}

pub struct MatchStartChannel; // Reliable channel used for match start notification.

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Event)] pub struct MatchLoadingNotification { // Match loading notification (server -> client), tells client can enter LoadingMatch state
	pub match_id: u64, // Match ID
	pub room_id: u64, // Fixed room number
}

pub struct MatchLoadingChannel; // Reliable channel used for match loading notification.

impl Default for LobbyBackendConfig {
	fn default() -> Self {
		Self {
			base_url: env_string("GAME_BACKEND_BASE_URL", "http://127.0.0.1:8080"),
		}
	}
}

#[derive(Debug, Deserialize)]
struct ValidateEntryTokenResponse {  // Response after lobby validates entry token
	success: bool, // Whether token is valid
	#[serde(rename = "userId")] user_id: Option<u64>, // Player ID corresponding to token
	#[serde(rename = "matchId")] match_id: Option<u64>, // Match ID corresponding to token
	#[serde(rename = "roomId")] room_id: Option<u64>, // Fixed room number corresponding to token
	#[serde(rename = "netcodeClientId")] netcode_client_id: Option<u64>, // Netcode client id assigned by backend.
	message: Option<String>, // Failure reason
}


#[derive(Component)]
struct PendingJoinValidationTask {  // Temporary task entity during room validation
	connection_entity: Entity, // Connection entity waiting to be added to room
	remote_id: PeerId, // Lightyear assigned remote player identifier
	request: JoinMatchRequest, // Client room join request
	task: Task<Result<ValidateEntryTokenResponse, String>>, // Lobby token validation task
}

#[derive(Component)]
struct PendingLoadNotification {  // Waiting to send load notification
	match_id: u64, // Match ID to notify client to load.
	room_id: u64, // Room number to notify client to load.
}


//region plugin
pub struct ConnectionPlugin {
    pub side: VleueSide, // Current running side, for dispatching client/server connection plugins.
}

impl Plugin for ConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ConnectionShaderPlugin); // Shared messages, channels and sync components registered first.
        match self.side {
            VleueSide::Client => app.add_plugins(ConnectionClientPlugin),
            VleueSide::Server => app.add_plugins(ConnectionServerPlugin),
        };
    }
}
//endregion plugin


//region shader
#[derive(Clone)]
pub struct ConnectionShaderPlugin;

impl Plugin for ConnectionShaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<GameRoomId>();
        app.register_component::<MatchId>();
        app.register_component::<PendingMatchServerConnection>();
        app.register_message::<JoinMatchRequest>().add_direction(NetworkDirection::ClientToServer);
        app.add_channel::<JoinMatchChannel>(ChannelSettings { mode: ChannelMode::OrderedReliable(ReliableSettings::default()), ..default() }).add_direction(NetworkDirection::ClientToServer);
        app.register_message::<MatchStartNotification>().add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<MatchStartChannel>(ChannelSettings { mode: ChannelMode::OrderedReliable(ReliableSettings::default()), ..default() }).add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchLoadingNotification>().add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<MatchLoadingChannel>(ChannelSettings { mode: ChannelMode::OrderedReliable(ReliableSettings::default()), ..default() }).add_direction(NetworkDirection::ServerToClient);
    }
}
//endregion shader



//region server

pub struct ConnectionServerPlugin;

impl Plugin for ConnectionServerPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<LobbyBackendConfig>(); // Server validates JoinMatchRequest, accesses lobby backend /api/match/validate address config.
		app.add_observer(handle_new_client_link);
		app.add_observer(record_server_linked);
		app.add_observer(record_server_connecting);
		app.add_observer(record_connected_client);
		app.add_observer(record_server_disconnected);
		app.add_observer(record_server_unlinked);
		app.add_systems(Update, (handle_join_match_requests, process_join_validation_tasks, send_load_notifications, update_match_state, spawn_pending_players).chain()); //.chain methods execute left to right in order
	}
}

fn handle_new_client_link(trigger: On<Add, LinkOf>, mut commands: Commands) { // Handle new client connection link
	debug!("[server] LinkOf added entity={:?}", trigger.entity);
	commands.entity(trigger.entity).insert((  // Enable this client connection to receive replicated data from server. Parameters: how often server sends replication updates to this client; only send replicated data changed since client's last ack
		ReplicationSender::new(SEND_INTERVAL, SendUpdatesMode::SinceLastAck, false),
	));
}

fn record_server_linked(trigger: On<Add, Linked>) { // Server underlying link established, commonly used to confirm UDP server itself started.
	debug!("[server] Linked added entity={:?}", trigger.entity);
}

fn record_server_connecting(trigger: On<Add, Connecting>) { // Server connection entity entering connecting state, mainly for debugging handshake phase.
	debug!("[server] Connecting added entity={:?}", trigger.entity);
}

fn record_connected_client(trigger: On<Add, Connected>, query: Query<Option<&RemoteId>, With<ClientOf>>, ) { // Record connected client, but only truly enter room after lobby token validation
	match query.get(trigger.entity) {
		Ok(Some(remote_id)) => info!("[server] client {:?} connected, waiting for JoinMatchRequest entity={:?}", remote_id.0, trigger.entity),
		Ok(None) => warn!("[server] Connected added but RemoteId missing entity={:?}", trigger.entity),
		Err(e) => error!("[server] Connected added but entity is not ClientOf entity={:?}, error={}", trigger.entity, e),
	}
}

fn record_server_disconnected(trigger: On<Add, Disconnected>, query: Query<&Disconnected>) { // Record server connection disconnect reason, including timeout, active disconnect and default unconnected state.
	let reason = query.get(trigger.entity).ok().and_then(|disconnected| disconnected.reason.as_deref()).unwrap_or("none");
	info!("[server] Disconnected added entity={:?}, reason={}", trigger.entity, reason);
}

fn record_server_unlinked(trigger: On<Add, Unlinked>, query: Query<&Unlinked>) { // Record underlying link disconnect reason, for distinguishing transport layer and Netcode layer issues.
	let reason = query.get(trigger.entity).map(|unlinked| unlinked.reason.as_str()).unwrap_or("unknown");
	info!("[server] Unlinked added entity={:?}, reason={}", trigger.entity, reason);
}

fn handle_join_match_requests(mut commands: Commands, players: Query<&VleueClientId, With<VleuePlayer>>, mut connections: Query<(Entity, &RemoteId, &mut MessageReceiver<JoinMatchRequest>), With<ClientOf>>, pending_tasks: Query<&PendingJoinValidationTask>, backend: Res<BackendClient>, ) { // Receive client room join request, and async call lobby backend to validate entry_token.
	for (connection_entity, remote_id, mut receiver) in &mut connections {
		for request in receiver.receive() {
			if players.iter().any(|existing| existing.0 == remote_id.0) {
				warn!("[server] duplicate JoinMatchRequest ignored for {:?}", remote_id.0);
				continue;
			}
			if pending_tasks.iter().any(|task| task.connection_entity == connection_entity) {
				warn!("[server] JoinMatchRequest already validating for {:?}", remote_id.0);
				continue;
			}

			let http = backend.http.clone();
			let base_url = backend.base_url.clone();
			let request_clone = request.clone();
			let task = IoTaskPool::get().spawn(async move {
				let url = format!("{}/api/match/validate", base_url);
				let response = http.post(url).json(&serde_json::json!({ "token": &request_clone.entry_token })).send();
				match response {
					Ok(resp) => match resp.json::<ValidateEntryTokenResponse>() {
						Ok(data) => Ok(data),
						Err(e) => Err(format!("Parsing failed: {}", e)),
					},
					Err(e) => Err(format!("Request failed: {}", e)),
				}
			});
			commands.spawn(PendingJoinValidationTask { connection_entity, remote_id: remote_id.0, request: request.clone(), task, });
			info!("[server] validating join token for {:?}, requested match {}, room {}", remote_id.0, request.match_id, request.room_id);
		}
	}
}

fn process_join_validation_tasks(mut commands: Commands, mut pending_tasks: Query<(Entity, &mut PendingJoinValidationTask)>, mut pending: ResMut<PendingPlayers>, room_manager: Res<RoomManager>, mut room_states: ResMut<MatchRoomStates>) { // Process lobby token validation result, add connection to room after validation passes and wait to spawn player.
	for (task_entity, mut pending_task) in &mut pending_tasks {
		let Some(result) = future::block_on(future::poll_once(&mut pending_task.task)) else { continue; };

		match result {
			Ok(validated) => {
				if !validated.success {
					warn!(
						"[server] lobby rejected token for {:?}: {}",
						pending_task.remote_id,
						validated.message.unwrap_or_else(|| "unknown reason".to_string())
					);
					commands.trigger(Disconnect { entity: pending_task.connection_entity });
					commands.entity(task_entity).despawn();
					continue;
				}

				let (Some(validated_user_id), Some(validated_match_id), Some(validated_room_id)) = (validated.user_id, validated.match_id, validated.room_id) else {
					warn!("[server] lobby validation response missing user_id, match_id or room_id");
					commands.trigger(Disconnect { entity: pending_task.connection_entity });
					commands.entity(task_entity).despawn();
					continue;
				};

				let expected_peer = validated.netcode_client_id.map(PeerId::Netcode).unwrap_or_else(|| {
					warn!("[server] lobby validation response missing netcode_client_id, falling back to user_id");
					PeerId::Netcode(validated_user_id)
				});
				if pending_task.remote_id != expected_peer || pending_task.request.player_id != validated_user_id || pending_task.request.match_id != validated_match_id || pending_task.request.room_id != validated_room_id {
					warn!("[server] join request mismatch: remote={:?}, expected_remote={:?}, request_player={}, validated_user={}, request_match={}, validated_match={}, request_room={}, validated_room={}, validated_netcode_client_id={:?}", pending_task.remote_id, expected_peer, pending_task.request.player_id, validated_user_id, pending_task.request.match_id, validated_match_id, pending_task.request.room_id, validated_room_id, validated.netcode_client_id);
					commands.trigger(Disconnect { entity: pending_task.connection_entity });
					commands.entity(task_entity).despawn();
					continue;
				}

				let Some(&room_entity) = room_manager.rooms.get(&validated_room_id) else {
					warn!("[server] room {} is not a fixed room, allowed range: 1..={}", validated_room_id, FIXED_ROOM_COUNT);
					commands.trigger(Disconnect { entity: pending_task.connection_entity });
					commands.entity(task_entity).despawn();
					continue;
				};

				// Add this client connection to room, enabling it to receive room network messages
				commands.trigger(RoomEvent {
					target: RoomTarget::AddSender(pending_task.connection_entity),
					room: room_entity,
				});

				// Mark this connection needs to send load notification
				commands.entity(pending_task.connection_entity).insert(PendingLoadNotification { match_id: validated_match_id, room_id: validated_room_id });
				let index = pending.players.len() as f32;
				let spawn_position = Vec2::new(index * 24.0, 0.0);
				pending.players.push((pending_task.connection_entity, pending_task.remote_id, spawn_position, validated_room_id, validated_match_id));
				room_states.register_player(validated_room_id, validated_match_id);
				info!("[server] client {:?} admitted to match {}, room {}", pending_task.remote_id, validated_match_id, validated_room_id);
			}
			Err(e) => {
				warn!("[server] token validation request failed for {:?}: {}", pending_task.remote_id, e);
				commands.trigger(Disconnect { entity: pending_task.connection_entity });
			}
		}

		commands.entity(task_entity).despawn();
	}
}

fn update_match_state(time: Res<Time>, mut room_states: ResMut<MatchRoomStates>) { // Update match state
	room_states.update_all(time.delta_secs());
}

fn send_load_notifications(mut commands: Commands, mut connections: Query<(Entity, &PendingLoadNotification, &mut MessageSender<MatchLoadingNotification>)>, ) {// Send load notification to client
	for (entity, pending, mut sender) in &mut connections {
		sender.send::<MatchLoadingChannel>(MatchLoadingNotification { match_id: pending.match_id, room_id: pending.room_id });
		debug!("[server] sent MatchLoadingNotification to {:?}", entity);
		commands.entity(entity).remove::<PendingLoadNotification>();
	}
}


fn spawn_pending_players(mut commands: Commands, mut pending: ResMut<PendingPlayers>, room_states: Res<MatchRoomStates>, room_manager: Res<RoomManager>) { // Spawn waiting players
	if pending.players.is_empty() {
		return;
	}
	let mut waiting_players = Vec::new();
	for (connection_entity, client_id, spawn_position, room_id, match_id) in pending.players.drain(..) {
		if !room_states.can_spawn_players(room_id) {
			waiting_players.push((connection_entity, client_id, spawn_position, room_id, match_id));
			continue;
		}
		info!("[server] spawning player {:?} at {:?} in match {}, room {}", client_id, spawn_position, match_id, room_id);
		let player_entity = commands.spawn((
			Name::new(format!("Player_{:?}", client_id)),
			VleuePlayer,
			VleueClientId(client_id),
			GameRoomId(room_id),
			MatchId(match_id),
			// Transform::from_translation(Vec3::new(spawn_position.x, -1., spawn_position.y)),
			Transform::from_translation(Vec3::new(10., 2., -10.)),
			ControlledBy {
				owner: connection_entity,
				lifetime: Lifetime::SessionBased,
			},
			Replicate::to_clients(NetworkTarget::All),
			DisableReplicateHierarchy, // Make child objects no longer network synced
			PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
			InterpolationTarget::to_clients(NetworkTarget::AllExceptSingle(client_id)),
		)).id();
		if let Some(&room_entity) = room_manager.rooms.get(&room_id) {// Add player entity to room isolation
			commands.trigger(RoomEvent {
				target: RoomTarget::AddEntity(player_entity),
				room: room_entity,
			});
		}
	}
	pending.players = waiting_players;
}




//endregion server







//region client

#[derive(Clone)]
pub struct ConnectionClientPlugin;

impl Plugin for ConnectionClientPlugin {
	fn build(&self, app: &mut App) {
		let in_game = in_state(AppClientState::InGame);
		app.init_resource::<MatchData>(); // Client caches match result and current battle server connection entity.
		app.add_observer(log_match_client_linked);
		app.add_observer(log_match_client_connecting);
		app.add_observer(log_match_client_connected);
		app.add_observer(log_match_client_disconnected);
		app.add_observer(log_match_client_unlinked);
		app.add_systems(Update, (connect_pending_match_servers, send_pending_join_match_requests).run_if(in_game.clone()).chain());
		app.add_systems(Update, cleanup_failed_match_connections.run_if(in_game.clone()));
		app.add_systems(Update, handle_match_loading_notification.run_if(in_game.clone()));
	}
}

pub fn connect_to_match_server(commands: &mut Commands<'_, '_>, match_data: &mut MatchData, netcode_client_id: u64) {  // Create connection info with game server
	if match_data.connection_started {
		return;
	}
	let (Some(host), Some(port)) = (&match_data.server_host, match_data.server_port) else {
		return;
	};
	let Ok(ip) = host.parse::<Ipv4Addr>() else {
		warn!("Battle server address is not valid IPv4: {}", host);
		return;
	};
	match_data.connection_started = true;
	let server_addr = SocketAddr::new(ip.into(), port as u16);
	let entity = commands.spawn((
		ClientConnection {
			netcode_client_id,
			client_port: 0,
			server_addr,
			conditioner: None,
			transport: ClientTransports::Udp,
			shared: shared_settings(),
		},
		PendingMatchServerConnection::default(),
	)).id();
	match_data.connection_entity = Some(entity);
	info!("[client] spawned match connection entity={:?}, server={}", entity, server_addr);
}

pub fn connect_pending_match_servers(mut commands: Commands, mut clients: Query<(Entity, &mut PendingMatchServerConnection), (With<NetworkClient>, Without<Connecting>, Without<Connected>)>) { // Trigger Lightyear Connect for newly created battle server connection entities.
	for (entity, mut pending) in &mut clients {
		if pending.connect_requested {
			continue;
		}
		pending.connect_requested = true;
		info!("[client] initiating connection");
		commands.trigger(Connect { entity });
	}
}

pub fn log_match_client_linked(trigger: On<Add, Linked>) { // Client underlying link established, doesn't mean Netcode handshake completed.
	debug!("[client] Linked added entity={:?}", trigger.entity);
}

pub fn log_match_client_connecting(trigger: On<Add, Connecting>) { // Client entering Netcode/Lightyear connecting state.
	debug!("[client] Connecting added entity={:?}", trigger.entity);
}

pub fn log_match_client_connected(trigger: On<Add, Connected>, query: Query<Option<&RemoteId>, With<NetworkClient>>, mut next_ingame_state: ResMut<NextState<InGameConnectionState>>) { // Netcode handshake completed, can send JoinMatchRequest.
	match query.get(trigger.entity) {
		Ok(Some(remote_id)) => info!("[client] Connected added entity={:?}, remote={:?}", trigger.entity, remote_id.0),
		Ok(None) => warn!("[client] Connected added entity={:?}, RemoteId missing", trigger.entity),
		Err(e) => error!("[client] Connected added entity={:?}, but it is not NetworkClient: {}", trigger.entity, e),
	}
	next_ingame_state.set(InGameConnectionState::Connected);
}

pub fn log_match_client_disconnected(trigger: On<Add, Disconnected>, query: Query<&Disconnected>, pending_query: Query<(), With<PendingMatchServerConnection>>, mut match_data: ResMut<MatchData>, mut next_ingame_state: ResMut<NextState<InGameConnectionState>>) { // Reset current battle server connection on connection failure or disconnect, enter reconnect state.
	let reason = query.get(trigger.entity).ok().and_then(|disconnected| disconnected.reason.as_deref()).unwrap_or("none");
	info!("[client] Disconnected added entity={:?}, reason={}", trigger.entity, reason);
	if reason != "none" && pending_query.contains(trigger.entity) {
		match_data.connection_started = false;
		if match_data.connection_entity == Some(trigger.entity) {
			match_data.connection_entity = None;
		}
		next_ingame_state.set(InGameConnectionState::Reconnecting);
	}
}

pub fn log_match_client_unlinked(trigger: On<Add, Unlinked>, query: Query<&Unlinked>, pending_query: Query<(), With<PendingMatchServerConnection>>, mut match_data: ResMut<MatchData>, mut next_ingame_state: ResMut<NextState<InGameConnectionState>>) { // Reset connection data when underlying transport disconnects, reconnect flow will re-request match status.
	let reason = query.get(trigger.entity).map(|unlinked| unlinked.reason.as_str()).unwrap_or("unknown");
	info!("[client] Unlinked added entity={:?}, reason={}", trigger.entity, reason);
	if pending_query.contains(trigger.entity) {
		match_data.connection_started = false;
		if match_data.connection_entity == Some(trigger.entity) {
			match_data.connection_entity = None;
		}
		next_ingame_state.set(InGameConnectionState::Reconnecting);
	}
}

pub fn cleanup_failed_match_connections(mut commands: Commands, failed_clients: Query<(Entity, &Disconnected), (With<PendingMatchServerConnection>, Without<Connected>)>) { // Cleanup truly failed connection entities, reason=none is initial unconnected state cannot cleanup.
	for (entity, disconnected) in &failed_clients {
		let reason = disconnected.reason.as_deref().unwrap_or("none");
		if reason == "none" {
			continue;
		}
		warn!("[client] cleanup failed match connection entity={:?}, reason={}", entity, reason);
		commands.entity(entity).despawn();
	}
}

pub fn send_pending_join_match_requests(match_data: Res<MatchData>, user_id: Res<UserId>, mut clients: Query<(Entity, &mut PendingMatchServerConnection, &mut MessageSender<JoinMatchRequest>), (With<NetworkClient>, With<Connected>)>, mut next_ingame_state: ResMut<NextState<InGameConnectionState>>) { //Connect to server to validate if token is correct
	let (Some(match_id), Some(room_id), Some(entry_token)) = (match_data.match_id, match_data.room_id, match_data.entry_token.clone()) else {
		return;
	};
	for (entity, mut pending, mut sender) in &mut clients {
		if pending.join_request_sent {
			continue;
		}
		pending.join_request_sent = true;
		debug!("[client] sending validation request entity={:?} roomid {}", entity, room_id);
		sender.send::<JoinMatchChannel>(JoinMatchRequest { match_id, room_id, player_id: user_id.0, entry_token: entry_token.clone() });
		next_ingame_state.set(InGameConnectionState::Joining);
	}
}

pub fn handle_match_loading_notification(current_ingame_state: Res<State<InGameState>>, mut next_connection_state: ResMut<NextState<InGameConnectionState>>, mut next_ingame_state: ResMut<NextState<InGameState>>, mut client: Query<&mut MessageReceiver<MatchLoadingNotification>, With<NetworkClient>>, ) { // Enter match Loading after receiving server room admission confirmation, keep original match phase when reconnecting.
	for mut receiver in &mut client {
		for notification in receiver.receive() {
			info!("[client] received MatchLoadingNotification match={} room={}", notification.match_id, notification.room_id);
			next_connection_state.set(InGameConnectionState::Connected);
			if *current_ingame_state.get() == InGameState::WaitingForConnection {
				next_ingame_state.set(InGameState::Loading);
			}
		}
	}
}





//endregion client

