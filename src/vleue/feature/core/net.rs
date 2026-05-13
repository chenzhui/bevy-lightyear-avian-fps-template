use std::time::Duration;
use bevy::app::{App, Plugin};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::{Commands, Component, Entity, Message, Query, Resource, With};
use bevy::prelude::error;
use bevy::tasks::Task;
use futures_lite::future;
use reqwest::blocking::Client;
use serde_json::Value;
use crate::vleue::feature::VleueSide;
use crate::vleue::util::env::env_string;
//region shader

pub const DEFAULT_BACKEND_BASE_URL: &str = "http://127.0.0.1:8080"; // Default lobby backend HTTP address, overridden by env when present.

#[derive(Resource,Clone)]
pub struct BackendClient{
    pub  http: Client,
    pub  base_url: String,
}

impl BackendClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            http: Client::builder().timeout(Duration::from_secs(10)).build().unwrap_or_default(),
            base_url: base_url.into(),
        }
    }

    pub fn url(&self) -> &str {
        &self.base_url
    }
}


impl Default for BackendClient {
    fn default() -> Self {
        Self::new(env_string("GAME_BACKEND_BASE_URL", DEFAULT_BACKEND_BASE_URL))
    }
}

#[derive(Clone)]
pub struct NetShaderPlugin;
pub struct NetPlugin {
    pub side: VleueSide, // net leaf entry: currently only registers common HTTP capability.
}

impl Plugin for NetShaderPlugin{
    fn build(&self, app: &mut App) {
        app.insert_resource(BackendClient::default());
        app.add_message::<HttpResponseEvent>();
    }
}

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let _ = self.side; // Reserve side for future expansion of different client network strategies without changing callers.
        app.add_plugins(NetShaderPlugin);
        app.add_systems(bevy::app::Update, poll_all_http_tasks); // Global HTTP task polling is only registered once, avoiding multiple gameplay leaves repeatedly polling the same Task.
    }
}

//endregion shader


/// Common HTTP task component: mounted on a temporary entity
#[derive(Component)]
pub struct HttpTask {
    pub action: &'static str, // Request action name, e.g. "JoinMatch", "CancelMatch"
    pub task: Task<bevy::prelude::Result<Value, String>>, // Use generic Value to receive any JSON
}

/// Common HTTP response event: triggered when any API returns
#[derive(Message, Debug)]
pub struct HttpResponseEvent {
    pub action: &'static str,
    pub data: Value, // Generic JSON object
}

pub type HttpTaskResult = bevy::prelude::Result<Value, String>;

pub fn spawn_http_task(commands: &mut Commands<'_, '_>, action: &'static str, task: Task<HttpTaskResult>) {
    commands.spawn(HttpTask { action, task });
}

/// Global auto-poller: automatically processes all API tasks
pub fn poll_all_http_tasks(mut commands: Commands, mut tasks: Query<(Entity, &mut HttpTask)>, mut event_writer: MessageWriter<HttpResponseEvent>) {
    for (entity, mut http_task) in tasks.iter_mut() {
        if let Some(result) = future::block_on(future::poll_once(&mut http_task.task)) {
            match result {
                Ok(data) => {
                    event_writer.write(HttpResponseEvent {
                        action: http_task.action,
                        data,
                    });
                }
                Err(e) => error!("API [{}] request failed: {}", http_task.action, e),
            }
            commands.entity(entity).despawn();
        }
    }
}

pub fn has_pending_http_action(tasks: &Query<'_, '_, &HttpTask>, action: &str) -> bool {
    tasks.iter().any(|task| task.action == action)
}

pub fn clear_http_tasks(commands: &mut Commands<'_, '_>, tasks: &Query<'_, '_, (Entity, &HttpTask)>, actions: &[&str]) {
    for (entity, task) in tasks.iter() {
        if actions.contains(&task.action) {
            commands.entity(entity).despawn();
        }
    }
}

pub fn cleanup_pending_http_tasks(mut commands: Commands, tasks: Query<Entity, With<HttpTask>>) {
    for entity in tasks.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn json_bool(data: &Value, key: &str) -> bool {
    data.get(key).and_then(Value::as_bool).unwrap_or(false)
}

pub fn json_i32(data: &Value, key: &str) -> i32 {
    data.get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(0)
}

pub fn json_u32(data: &Value, key: &str) -> Option<u32> {
    data.get(key).and_then(Value::as_u64).and_then(|value| u32::try_from(value).ok())
}

pub fn json_u64(data: &Value, key: &str) -> Option<u64> {
    data.get(key).and_then(Value::as_u64)
}

pub fn json_string(data: &Value, key: &str) -> Option<String> {
    data.get(key).and_then(Value::as_str).map(str::to_string)
}
