use std::sync::atomic::{AtomicU64, Ordering};

use crate::vleue::util::env::env_u32;
use crate::vleue::util::log_util::LogThrottler;
use avian3d::PhysicsPlugins;
use avian3d::collision::CollisionDiagnostics;
use avian3d::dynamics::solver::schedule::SubstepCount;
use avian3d::interpolation::PhysicsInterpolationPlugin;
use avian3d::physics_transform::{PhysicsTransformPlugin, Position, Rotation};
use avian3d::prelude::{AngularVelocity, ComputedMass, LayerMask, LinearVelocity};
use bevy::prelude::*;
use lightyear::avian3d::plugin::{AvianReplicationMode, LightyearAvianPlugin};
use lightyear::prelude::{InterpolationRegistrationExt, PredictionRegistrationExt};
use lightyear_replication::prelude::AppComponentExt;


pub const WORLD_LAYER: LayerMask = LayerMask(1 << 0); // World static collision layer, ground and scene obstacles go here.
pub const PLAYER_LAYER: LayerMask = LayerMask(1 << 1); // Player movement collision layer, only blocks character vs world.
pub const HITBOX_LAYER: LayerMask = LayerMask(1 << 2); // Hitbox query layer, reserved for shooting hit detection.

pub struct VleuePhysicsPlugin;

#[derive(Resource)]
struct PhysicsTelemetry {
    fixed_steps_this_frame: u32,  // Number of fixed timestep steps executed in the current frame
    max_fixed_steps_seen: u32,  // Maximum number of fixed steps observed in a single frame since startup
    multi_fixed_frames: u64,  // Total count of frames that executed more than one fixed step
}

static POSITION_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static ROTATION_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static LINEAR_VELOCITY_ROLLBACKS: AtomicU64 = AtomicU64::new(0);
static ANGULAR_VELOCITY_ROLLBACKS: AtomicU64 = AtomicU64::new(0);

impl Plugin for VleuePhysicsPlugin {
    fn build(&self, app: &mut App) {
        let physics_substeps = configured_physics_substeps();
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
        app.add_plugins(PhysicsPlugins::default().build().disable::<PhysicsTransformPlugin>().disable::<PhysicsInterpolationPlugin>());
        //.set(PhysicsDebugPlugin::default())
        app.insert_resource(SubstepCount(physics_substeps));
        info!("[physics] Avian substeps={} (set GAME_PHYSICS_SUBSTEPS to override)",physics_substeps);

        // Register physics component synchronization, prediction and interpolation
	    app.register_component::<Position>().add_prediction().add_should_rollback(position_should_rollback).add_linear_correction_fn().add_linear_interpolation();
	    app.register_component::<Rotation>().add_prediction().add_should_rollback(rotation_should_rollback).add_linear_correction_fn().add_linear_interpolation();
	    app.register_component::<LinearVelocity>().add_prediction().add_should_rollback(linear_velocity_should_rollback);
	    app.register_component::<AngularVelocity>().add_prediction().add_should_rollback(angular_velocity_should_rollback);
	    app.register_component::<ComputedMass>().add_prediction();
      app.insert_resource(PhysicsTelemetry { fixed_steps_this_frame: 0, max_fixed_steps_seen: 0,multi_fixed_frames: 0, });
      app.add_systems(FixedFirst, count_fixed_step_for_telemetry);
      app.add_systems(Update, log_physics_telemetry); // Shows fixed-step catch-up and rollback pressure when FPS drops.
    }
}

fn configured_physics_substeps() -> u32 {
    env_u32("GAME_PHYSICS_SUBSTEPS", 2).clamp(1, 12)
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

fn count_fixed_step_for_telemetry(mut telemetry: ResMut<PhysicsTelemetry>) {
    telemetry.fixed_steps_this_frame += 1;
}

fn log_physics_telemetry(mut log_throttler: ResMut<LogThrottler>, time: Res<Time>, fixed_time: Res<Time<Fixed>>, mut telemetry: ResMut<PhysicsTelemetry>, substeps: Res<SubstepCount>, collision_diagnostics: Option<Res<CollisionDiagnostics>>, ) {
    let position_rollbacks = POSITION_ROLLBACKS.swap(0, Ordering::Relaxed);
    let rotation_rollbacks = ROTATION_ROLLBACKS.swap(0, Ordering::Relaxed);
    let linear_velocity_rollbacks = LINEAR_VELOCITY_ROLLBACKS.swap(0, Ordering::Relaxed);
    let angular_velocity_rollbacks = ANGULAR_VELOCITY_ROLLBACKS.swap(0, Ordering::Relaxed);
    let collision_count = collision_diagnostics.map(|diagnostics| diagnostics.contact_count).unwrap_or(0);
    let prediction_corrections = position_rollbacks + rotation_rollbacks + linear_velocity_rollbacks + angular_velocity_rollbacks;
    let fixed_steps_this_frame = telemetry.fixed_steps_this_frame;
    if fixed_steps_this_frame > 1 {
        telemetry.multi_fixed_frames += 1;
    }
    telemetry.max_fixed_steps_seen = telemetry.max_fixed_steps_seen.max(fixed_steps_this_frame);
    let solver_substeps_this_frame = fixed_steps_this_frame.saturating_mul(substeps.0);
    let fixed_overstep_ticks = fixed_time.overstep_fraction_f64();

		//[wzh_TODO]template not log
    // log_throttler.log("physics_perf", || {
	  //   info!("[physics perf] frame_dt_ms={:.2} fixed_steps/frame={} solver_substeps/frame={} max_fixed_steps={} multi_fixed_frames={} fixed_overstep_ticks={:.2} contacts={} rollbacks/window={} (pos={}, rot={}, lin_vel={}, ang_vel={})",time.delta_secs_f64() * 1000.0,fixed_steps_this_frame,
		// 	solver_substeps_this_frame,telemetry.max_fixed_steps_seen,telemetry.multi_fixed_frames,fixed_overstep_ticks,collision_count,prediction_corrections,position_rollbacks,
		// 	rotation_rollbacks,linear_velocity_rollbacks,angular_velocity_rollbacks,
		// );
    // });

    telemetry.fixed_steps_this_frame = 0;
}
