
pub mod feature;
pub mod cli;
pub mod cli_connection;
pub mod util;

use clap::Parser;
use core::time::Duration;
use bevy::app::{App, Plugin};
use bevy::prelude::Name;
use lightyear_replication::prelude::AppComponentExt;
use crate::vleue::cli::{Cli, Mode};
use crate::vleue::cli_connection::FIXED_TIMESTEP_HZ;
use crate::vleue::feature::{FeaturePlugin, VleueSide};

use crate::vleue::util::env::load_dotenv;
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
    let mut app = cli.build_app(Duration::from_secs_f64(1.0 / FIXED_TIMESTEP_HZ));  // Will add the window
    let side = match mode {
        Some(Mode::Client { .. }) => Some(VleueSide::Client),
        Some(Mode::Server { .. }) => Some(VleueSide::Server),
        _ => None,
    };
    if let Some(side) = side {
        app.add_plugins(VleuePlugin { side, headless: cli.is_headless_server() });
    }
    cli.spawn_connections(&mut app);  // Create service / establish connection
    app.run();
}

