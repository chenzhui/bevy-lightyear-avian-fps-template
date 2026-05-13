use bevy::prelude::*;
use crate::vleue::feature::core::state::{AppClientState, LobbyState, UserId};
use crate::vleue::feature::core::i18n::I18nResource;
use crate::vleue::feature::core::connection::MatchData;
use crate::vleue::feature::core::net::{BackendClient, HttpTask};
use bevy_inspector_egui::bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use crate::vleue::feature::lobby::matching::{cancel_matchmaking, start_matchmaking, ACTION_CANCEL_MATCH, QueueUiState};

pub struct LobbyUiClientPlugin;

impl Plugin for LobbyUiClientPlugin {
	fn build(&self, app: &mut App) {
		let in_lobby = in_state(AppClientState::Lobby);
		app.add_systems(EguiPrimaryContextPass, draw_lobby_ui.run_if(in_lobby.clone().and(in_lobby_main_state)));
	}
}

fn in_lobby_main_state(lobby_state: Option<Res<State<LobbyState>>>) -> bool {
	let Some(lobby_state) = lobby_state else { return false; };
	matches!(lobby_state.get(), LobbyState::Idle | LobbyState::Queuing)
}

pub fn draw_lobby_ui(mut contexts: EguiContexts, mut next_state: ResMut<NextState<LobbyState>>, match_state: Res<State<LobbyState>>, mut commands: Commands, client: Res<BackendClient>,
    mut match_data: ResMut<MatchData>, mut queue_state: ResMut<QueueUiState>, user_id: Res<UserId>, http_tasks: Query<(Entity, &HttpTask)>,
    i18n: Res<I18nResource>,
) {
	let Ok(ctx) = contexts.ctx_mut() else { return; };
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.heading(i18n.t("lobby.title"));
            ui.add_space(20.0);

            match match_state.get() {
                LobbyState::Idle => {
                    if ui.add_sized([200.0, 50.0], egui::Button::new(i18n.t("lobby.start_match"))).clicked() {
                        start_matchmaking(&mut commands, &client, user_id.0, 1500, &mut match_data, &mut queue_state, &http_tasks, &mut next_state);
                    }
                }
                LobbyState::Queuing => {
                    ui.label(i18n.t_args("lobby.queuing", &queue_state.current_queue_size.to_string()));
                    let cancelling = http_tasks.iter().any(|(_, task)| task.action == ACTION_CANCEL_MATCH);
                    if ui.add_enabled(!cancelling, egui::Button::new(i18n.t("lobby.cancel")).min_size(egui::vec2(150.0, 40.0))).clicked() {
                        cancel_matchmaking(&mut commands, &client, user_id.0, &http_tasks);
                    }
                }
                LobbyState::PostMatch => {}
            }
        });
    });
}
