use std::sync::atomic::{AtomicU64, Ordering};

use avian3d::collision::CollisionDiagnostics;
use avian3d::debug_render::PhysicsDebugPlugin;
use avian3d::interpolation::PhysicsInterpolationPlugin;
use avian3d::physics_transform::{PhysicsTransformPlugin, Position, Rotation};
use avian3d::PhysicsPlugins;
use avian3d::prelude::{AngularVelocity, ComputedMass, LinearVelocity};
use bevy::log::info;
use bevy::prelude::*;
use lightyear::avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use lightyear::prelude::{InterpolationRegistrationExt, PredictionRegistrationExt};
use lightyear_replication::prelude::AppComponentExt;
use crate::vleue::util::log_util::LogThrottler;

pub struct VleuePhysicsPlugin;

#[derive(Resource)]
struct PhysicsTelemetry {
	timer: Timer,
}

static POSITION_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static ROTATION_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static LINEAR_VELOCITY_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static ANGULAR_VELOCITY_ROLLBACKS: AtomicU64 = AtomicU64::new(0);

impl Plugin for VleuePhysicsPlugin {
	fn build(&self, app: &mut App) {
		app.add_plugins(LightyearAvianPlugin {
			replication_mode: AvianReplicationMode::Position,
			..default()
		});

        //     * PhysicsTransformPlugin: We disable it because we want the network replication system to be the only one writing to
        // the Transform components. If this plugin were active, it would fight with the network updates, leading to visual
        // flickering or jitter.
        //     * PhysicsInterpolationPlugin: We disable it because FPS synchronization needs its own specific logic to handle
        // network jitter. Enabling the engine's built-in interpolation would cause "double-smoothing," which adds unnecessary
        // input latency and makes movement feel heavy or inaccurate.
		app.add_plugins(PhysicsPlugins::default().build().disable::<PhysicsTransformPlugin>().disable::<PhysicsInterpolationPlugin>() );
        //.set(PhysicsDebugPlugin::default())

		// Register physics component synchronization, prediction and interpolation
		app.register_component::<Position>().add_prediction().add_should_rollback(position_should_rollback).add_linear_correction_fn().add_linear_interpolation();
		app.register_component::<Rotation>().add_prediction().add_should_rollback(rotation_should_rollback).add_linear_correction_fn().add_linear_interpolation();
		app.register_component::<LinearVelocity>().add_prediction().add_should_rollback(linear_velocity_should_rollback);
		app.register_component::<AngularVelocity>().add_prediction().add_should_rollback(angular_velocity_should_rollback);
		app.register_component::<ComputedMass>().add_prediction();
		app.insert_resource(PhysicsTelemetry {
			timer: Timer::from_seconds(1.0, TimerMode::Repeating),
		});
		app.add_systems(Update, log_physics_telemetry); // If FPS is low and you suspect position rollback causing complex collision calculations, enable this
	}
}

fn position_should_rollback(this: &Position, that: &Position) -> bool {
	let should_rollback = (this.0 - that.0).length() >= 0.01;
	if should_rollback {
		POSITION_ROLLBACKS.fetch_add(1, Ordering::Relaxed);
	}
	should_rollback
}

fn rotation_should_rollback(this: &Rotation, that: &Rotation) -> bool {
	let should_rollback = this.angle_between(*that) >= 0.01;
	if should_rollback {
		ROTATION_ROLLBACKS.fetch_add(1, Ordering::Relaxed);
	}
	should_rollback
}

fn linear_velocity_should_rollback(this: &LinearVelocity, that: &LinearVelocity) -> bool {
	let should_rollback = (this.0 - that.0).length() >= 0.01;
	if should_rollback {
		LINEAR_VELOCITY_ROLLBACKS.fetch_add(1, Ordering::Relaxed);
	}
	should_rollback
}

fn angular_velocity_should_rollback(this: &AngularVelocity, that: &AngularVelocity) -> bool {
	let should_rollback = (this.0 - that.0).length() >= 0.01;
	if should_rollback {
		ANGULAR_VELOCITY_ROLLBACKS.fetch_add(1, Ordering::Relaxed);
	}
	should_rollback
}

fn log_physics_telemetry(mut log_throttler: ResMut<LogThrottler>,time: Res<Time>, mut telemetry: ResMut<PhysicsTelemetry>, collision_diagnostics: Option<Res<CollisionDiagnostics>>, ) {
	// if !telemetry.timer.tick(time.delta()).just_finished() {
	// 	println!("??");
	// 	return;
	// }

	let position_rollbacks = POSITION_ROLLBACKS.swap(0, Ordering::Relaxed);
	let rotation_rollbacks = ROTATION_ROLLBACKS.swap(0, Ordering::Relaxed);
	let linear_velocity_rollbacks = LINEAR_VELOCITY_ROLLBACKS.swap(0, Ordering::Relaxed);
	let angular_velocity_rollbacks = ANGULAR_VELOCITY_ROLLBACKS.swap(0, Ordering::Relaxed);
	let collision_count = collision_diagnostics.map(|diagnostics| diagnostics.contact_count).unwrap_or(0);
	let prediction_corrections = position_rollbacks + rotation_rollbacks + linear_velocity_rollbacks + angular_velocity_rollbacks;
    log_throttler.log("666", || {
        debug!("[physics] contacts={} rollback/s={} (pos={}, rot={}, lin_vel={}, ang_vel={})", collision_count, prediction_corrections, position_rollbacks, rotation_rollbacks, linear_velocity_rollbacks, angular_velocity_rollbacks, );
    });
	
}
