use bevy::prelude::*;
use bevy::app::AppExit;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy_inspector_egui::bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use serde::{Deserialize, Serialize};
use crate::vleue::feature::core::net::HttpTask;
use crate::vleue::feature::VleueSide;
use crate::vleue::util::log_util::LogThrottler;

pub struct StateFeaturePlugin {
    pub side: VleueSide,
}

impl Plugin for StateFeaturePlugin {
    fn build(&self, app: &mut App) {
        if self.side.is_client() {
            debug!("client plugin");
            app.add_plugins(ClientStatePlugin);
        }
    }
}

//region server

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RoomPhaseState {/// Room phase
    #[default]
    Waiting, // Waiting for players to join
    InProgress, // Match in progress
    Ended, // Match ended
}


//endregion server

//region client

/// Client application state - controls current player's view
/// Client state is driven by server message switching
#[derive(States, Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum AppClientState {
	Startup,/// Game startup, initialization
	Login,/// Login interface (lobby server uses Java, temporarily skipped here)
	#[default]   // Temporary default while lobby login is optional.
	Lobby,/// Lobby interface (team, match waiting)
	InGame,// Game in progress
    Offline, // Fatal error or account offline error interface.
}

#[derive(SubStates, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
#[source(AppClientState = AppClientState::Lobby)]
pub enum LobbyState {
    #[default]
    Idle,
    Queuing,
    PostMatch,// Match end settlement
}

#[derive(SubStates, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
#[source(AppClientState = AppClientState::InGame)]
pub enum InGameConnectionState {
    #[default]
    Connecting, // Entered match state, connecting to battle server.
    Connected, // Lightyear Connected, waiting to send entry validation.
    Joining, // JoinMatchRequest sent, waiting for server room admission confirmation.
    Reconnecting, // Disconnected during InGame, waiting to re-fetch match status and reconnect.
    Failed, // Reconnect failed or backend no longer allows entry.
}

#[derive(SubStates, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
#[source(AppClientState = AppClientState::InGame)]
pub enum InGameState {
    #[default]
    WaitingForConnection, // Entered InGame, but battle server room admission not confirmed yet.
    Loading, // Server confirmed room admission, client loads match resources and waits for player entity.
    WaitingPlayers, // Local resources ready, waiting for other players or server start conditions.
    Countdown, // Match countdown.
    Playing, // In match.
    Ending, // Match ending flow, waiting for settlement data.
}

#[derive(Component, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum LocalPlayerState {
    #[default]
    Spawning, // Local player spawning or respawning.
    Alive, // Local player alive, can participate in match.
    Dead, // Local player dead, waiting for respawn or elimination.
    Spectating, // Local player spectating.
}

#[derive(Resource, Default)]
pub struct PendingLobbyState(pub Option<LobbyState>); // Cross AppClientState switch, sub-state applied after entering Lobby.

#[derive(Resource, Deref, DerefMut)]
pub struct UserId(pub u64); // Current client user ID, used for lobby HTTP and battle server Netcode client_id.

#[derive(Message, Clone, Debug)]
pub struct FatalErrorEvent {
	pub message: String, // Error reason that needs to interrupt all business processes and display to player.
}

#[derive(Resource, Default)]
pub struct PersistentErrorInfo {
	pub message: String, // Last fatal error displayed on Offline interface.
}

pub fn is_ingame_playing_and_connected(connection_state: Option<Res<State<InGameConnectionState>>>, ingame_state: Option<Res<State<InGameState>>>) -> bool { // Unified switch for match input and strong interaction systems, closed when SubState doesn't exist.
	let (Some(connection_state), Some(ingame_state)) = (connection_state, ingame_state) else { return false; };
	*connection_state.get() == InGameConnectionState::Connected && *ingame_state.get() == InGameState::Playing
}

/// Client state plugin
pub struct ClientStatePlugin;

impl Plugin for ClientStatePlugin {
	fn build(&self, app: &mut App) {
		app.init_state::<AppClientState>();
		app.add_sub_state::<LobbyState>();
		app.add_sub_state::<InGameConnectionState>();
		app.add_sub_state::<InGameState>();
		app.init_resource::<PendingLobbyState>();
		app.init_resource::<PersistentErrorInfo>();
		app.add_message::<FatalErrorEvent>();
		app.add_systems(Update, log_client_state);
		app.add_systems(Update, handle_fatal_errors);
		app.add_systems(OnEnter(AppClientState::Offline), cleanup_http_tasks_on_offline);
		app.add_systems(EguiPrimaryContextPass, draw_offline_ui.run_if(in_state(AppClientState::Offline)));
	}
}

fn handle_fatal_errors(mut events: MessageReader<FatalErrorEvent>, mut error_info: ResMut<PersistentErrorInfo>, mut next_state: ResMut<NextState<AppClientState>>) {
	for event in events.read() {
		error_info.message = event.message.clone();
		next_state.set(AppClientState::Offline);
	}
}

fn cleanup_http_tasks_on_offline(mut commands: Commands, tasks: Query<Entity, With<HttpTask>>) {
	for entity in &tasks {
		commands.entity(entity).despawn();
	}
}

fn draw_offline_ui(mut contexts: EguiContexts, error_info: Res<PersistentErrorInfo>, mut app_exit: MessageWriter<AppExit>) {
	let Ok(ctx) = contexts.ctx_mut() else { return; };
	egui::CentralPanel::default().show(ctx, |ui| {
		ui.vertical_centered(|ui| {
			ui.add_space(140.0);
			ui.heading("Offline");
			ui.add_space(12.0);
			let message = if error_info.message.is_empty() { "Connection interrupted, please restart client to refresh state." } else { error_info.message.as_str() };
			ui.label(message);
			ui.add_space(20.0);
			if ui.add_sized([140.0, 36.0], egui::Button::new("Exit Game")).clicked() {
				app_exit.write(AppExit::Success);
			}
		});
	});
}

/// Use LogThrottler to output client state (throttled to avoid spam)
fn log_client_state(state: Res<State<AppClientState>>, lobby_state: Option<Res<State<LobbyState>>>, ingame_connection_state: Option<Res<State<InGameConnectionState>>>, ingame_state: Option<Res<State<InGameState>>>, mut log_throttler: ResMut<LogThrottler>, ) {
	log_throttler.log("client_app_state", || {
		match state.get() {
			AppClientState::Lobby => {
				if let Some(lobby_state) = lobby_state.as_deref() {
					debug!("📊 [ClientAppState] Current state: {:?}, sub state: {:?}", state.get(), lobby_state.get());
				} else {
					debug!("📊 [ClientAppState] Current state: {:?}, sub state: None", state.get());
				}
			}
			AppClientState::InGame => {
				if let (Some(ingame_connection_state), Some(ingame_state)) = (ingame_connection_state.as_deref(), ingame_state.as_deref()) {
					debug!("📊 [ClientAppState] Current state: {:?}, connection state: {:?}, match state: {:?}", state.get(), ingame_connection_state.get(), ingame_state.get());
				} else {
					debug!("📊 [ClientAppState] Current state: {:?}, sub state: None", state.get());
				}
			}
			_ => {
				debug!("📊 [ClientAppState] Current state: {:?}", state.get());
			}
		}
	});
}
//endregion client
