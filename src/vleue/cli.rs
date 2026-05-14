use core::time::Duration;
use bevy::log::{BoxedFmtLayer, Level, LogPlugin};
use bevy::prelude::*;
use bevy::log::tracing_subscriber::prelude::*;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use clap::{Parser, Subcommand};

use lightyear::prelude::*;

use crate::vleue::cli_connection::{connect, server_port, shared_settings, why_start, ClientConnection, ClientTransports, ServerConnection, ServerTransports};
use crate::vleue::feature::core::{VleueWindowPlugin};
use crate::vleue::feature::core::server_debug::ServerDebugPlugin;
use crate::vleue::feature::core::state::UserId;
use crate::vleue::feature::VleueSide;

pub struct ProjectLogPlugin;

impl Plugin for ProjectLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LogPlugin {
            level: Level::INFO,
            filter: format!("info,wgpu=error,naga=warn,{}=trace", env!("CARGO_PKG_NAME")),
            fmt_layer: project_fmt_layer,
            ..default()
        });
    }
}

fn project_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(bevy::log::tracing_subscriber::filter::filter_fn(|metadata| {
                if metadata.fields().field("tracy.frame_mark").is_some() {
                    return false;
                }
                let target = metadata.target();
                let is_project_target = target == env!("CARGO_PKG_NAME") || target.starts_with(concat!(env!("CARGO_PKG_NAME"), "::"));
                is_project_target || matches!(*metadata.level(), Level::ERROR | Level::WARN)
            })),
    ))
}

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub mode: Option<Mode>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Mode {
    Client {
        #[arg(short, long, default_value = None)]
        client_id: Option<u64>,
        #[arg(long, default_value_t = false)]
        free_cam: bool,
    },
    Server {
        #[arg(short, long, default_value_t = 0)]
        room_id: u64,
    },
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct IsFreeCam(pub bool);

pub struct VleueCliPlugin {
    pub mode: Mode,
    pub tick_duration: Duration,
}

impl Plugin for VleueCliPlugin {
    fn build(&self, app: &mut App) {
        match &self.mode {
            Mode::Server { room_id } => {
                if *room_id!=0 {
                    info!("Server room id: {}", room_id);
                    app.add_plugins(ServerDebugPlugin { room_id: Some(*room_id) });
                }
                app.add_plugins(server::ServerPlugins { tick_duration: self.tick_duration });
                if *room_id != 0 {
                    app.add_plugins(VleueWindowPlugin {
                        side: VleueSide::Server,
                        name: format!("Server room {room_id}"),
                        server_room: Some(*room_id),
                    });
                }
            }
            Mode::Client { client_id, free_cam } => {
                app.insert_resource(IsFreeCam(*free_cam));
                app.insert_resource(UserId(client_id.unwrap_or(1)));
                app.add_plugins(client::ClientPlugins { tick_duration: self.tick_duration });
                app.add_plugins(VleueWindowPlugin {
                    side: VleueSide::Client,
                    name: format!("Client {client_id:?}"),
                    server_room: None,
                });
            }
        }
    }
}

fn asset_plugin_settings() -> AssetPlugin {
    AssetPlugin {
        file_path: format!("{}\\assets", env!("CARGO_MANIFEST_DIR")),
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..default()
    }
}

impl Cli {
    pub fn is_headless_server(&self) -> bool {
        if let Some(Mode::Server { room_id }) = self.mode {
            return room_id == 0;
        }
        false
    }

    pub fn build_app(&self, tick_duration: Duration) -> App {
        let mut app = App::new();
        if self.is_headless_server() {
            app.add_plugins(DefaultPlugins.build().set(asset_plugin_settings()).disable::<bevy::log::LogPlugin>().disable::<bevy::winit::WinitPlugin>());
            app.add_plugins(ProjectLogPlugin);
            app.add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(tick_duration));
        } else {
            app.add_plugins(DefaultPlugins.build().set(asset_plugin_settings()).disable::<bevy::log::LogPlugin>());
            app.add_plugins(ProjectLogPlugin);
            app.add_plugins(EguiPlugin::default());
        }

        if let Some(mode) = self.mode.clone() {
            app.add_plugins(VleueCliPlugin {
                mode,
                tick_duration,
            });
        }
        app
    }

    pub fn spawn_connections(&self, app: &mut App) {
        match self.mode.as_ref().unwrap() {
            Mode::Client { .. } => {
            }
            Mode::Server { .. } => {
                app.world_mut().spawn(ServerConnection {
                    conditioner: None,
                    transport: ServerTransports::Udp { local_port: server_port() },
                    shared: shared_settings(),
                });
                app.add_systems(Startup, why_start);
            }
        }
    }
}
