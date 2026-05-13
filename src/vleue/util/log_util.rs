use std::collections::HashMap;
use std::time::{Duration, Instant};
use bevy::log::info;
use bevy::prelude::{Res, ResMut, Resource, State};
//mut log_throttler: ResMut<LogThrottler>

#[derive(Resource, Default)]
pub struct LogThrottler {
    last_log_times: HashMap<String, Instant>,/// Stores the last output time for each ID
    throttle_duration: Duration, // Throttle interval (default 2 seconds)
}

impl LogThrottler {
    pub fn new(throttle_duration_secs: f64) -> Self {
        Self {
            last_log_times: HashMap::new(),
            throttle_duration: Duration::from_secs_f64(throttle_duration_secs),
        }
    }

    /// Check if log should be output (time since last output exceeds specified duration)
    pub fn should_log(&mut self, id: &str) -> bool {
        let now = Instant::now();

        if let Some(last_time) = self.last_log_times.get(id) {
            if now.duration_since(*last_time) >= self.throttle_duration {
                self.last_log_times.insert(id.to_string(), now);
                true
            } else {
                false
            }
        } else {
            // First output
            self.last_log_times.insert(id.to_string(), now);
            true
        }
    }

    /// Throttled log output (only execute closure when should output)
    pub fn log<F>(&mut self, id: &str, log_fn: F) where F: FnOnce(),{
        if self.should_log(id) {
            log_fn();
        }
    }

    /// Clean up expired records (optional, prevents memory leak)
    pub fn cleanup(&mut self) {
        let now = Instant::now();
        // Remove records unused for more than 10 seconds
        self.last_log_times.retain(|_, last_time| {
            now.duration_since(*last_time) < self.throttle_duration * 5
        });
    }
}

// Usage

// fn log_client_state(state: Res<State<ClientAppState>>, mut log_throttler: ResMut<LogThrottler>, ) {
//     log_throttler.log("client_app_state", || {
//         println!("📊 [ClientAppState] Current state: {:?}", state.get());
//     });
// }