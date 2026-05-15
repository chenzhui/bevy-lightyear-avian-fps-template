pub mod cli;
pub mod cli_connection;
pub mod feature;
pub mod util;

use crate::vleue::cli::{Cli, Mode};
use crate::vleue::cli_connection::FIXED_TIMESTEP_HZ;
use crate::vleue::feature::{FeaturePlugin, VleueSide};
use bevy::app::{App, Plugin};
use bevy::log::info;
use bevy::prelude::{Name, Time, Virtual};
use clap::Parser;
use core::time::Duration;
use lightyear_replication::prelude::AppComponentExt;

use crate::vleue::util::env::{env_f64, env_u32, load_dotenv};
use crate::vleue::util::log_util::LogThrottler;

pub struct VleuePlugin {
    pub side: VleueSide, // Root plugin: dispatches whether running on client or server.
    pub headless: bool,
}

impl Plugin for VleuePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LogThrottler::new(2.0)); // All sides reuse this log throttler.
        app.register_component::<Name>(); // Common network entity names registered at root level.
        app.add_plugins(FeaturePlugin { side: self.side, headless: self.headless });
    }
}

pub fn main() {
    load_dotenv();
    let cli = Cli::parse();
    let mode = cli.mode.clone();
    let fixed_timestep_hz = configured_fixed_timestep_hz();
    let tick_duration = Duration::from_secs_f64(1.0 / fixed_timestep_hz);
    let mut app = cli.build_app(tick_duration); // Will add the window
    configure_fixed_catch_up_limit(&mut app, tick_duration);
    let side = match mode {
        Some(Mode::Client { .. }) => Some(VleueSide::Client),
        Some(Mode::Server { .. }) => Some(VleueSide::Server),
        _ => None,
    };
    if let Some(side) = side {
        app.add_plugins(VleuePlugin { side, headless: cli.is_headless_server()});
    }
    cli.spawn_connections(&mut app); // Create service / establish connection
    app.run();
}

fn configured_fixed_timestep_hz() -> f64 {
    env_f64("GAME_FIXED_TIMESTEP_HZ", FIXED_TIMESTEP_HZ).clamp(10.0, 128.0)
}

fn configure_fixed_catch_up_limit(app: &mut App, tick_duration: Duration) {
    let max_fixed_steps = env_u32("GAME_MAX_FIXED_STEPS_PER_FRAME", 4).clamp(1, 8);
    let max_delta = tick_duration.saturating_mul(max_fixed_steps);
    app.world_mut().resource_mut::<Time<Virtual>>().set_max_delta(max_delta);
    info!("[time] fixed_timestep_hz={:.2} max_fixed_steps_per_frame={} max_virtual_delta_ms={:.2}",1.0 / tick_duration.as_secs_f64(),max_fixed_steps,max_delta.as_secs_f64() * 1000.0);
}
