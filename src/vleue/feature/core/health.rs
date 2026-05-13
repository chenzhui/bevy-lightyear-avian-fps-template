use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};
use bevy::prelude::*;
use crate::vleue::feature::VleueSide;

const DEFAULT_HEALTH_PORT: u16 = 5888;

#[derive(Resource, Clone, Debug)]
pub struct GameServerHealthConfig {
	pub bind_host: String, // Health check HTTP listening address.
	pub port: u16, // Health check HTTP listening port.
}

impl Default for GameServerHealthConfig {
	fn default() -> Self {
		let port = std::env::var("GAME_HEALTH_PORT").ok().and_then(|value| value.parse::<u16>().ok()).unwrap_or(DEFAULT_HEALTH_PORT);
		Self { bind_host: "0.0.0.0".to_string(), port }
	}
}

pub struct GameServerHealthPlugin {
	pub side: VleueSide,
}

impl Plugin for GameServerHealthPlugin {
	fn build(&self, app: &mut App) {
		if self.side.is_server() {
			app.init_resource::<GameServerHealthConfig>();
			app.add_systems(Startup, start_health_server);
		}
	}
}

fn start_health_server(config: Res<GameServerHealthConfig>) {
	let bind_addr = format!("{}:{}", config.bind_host, config.port);
	thread::spawn(move || {
		let started_at = Instant::now();
		let Ok(listener) = TcpListener::bind(&bind_addr) else {
			error!("[health] failed to bind {}", bind_addr);
			return;
		};
		info!("[health] listening on http://{}/health", bind_addr);
		for stream in listener.incoming() {
			match stream {
				Ok(mut stream) => handle_health_request(&mut stream, started_at),
				Err(err) => warn!("[health] accept failed: {}", err),
			}
		}
	});
}

fn handle_health_request(stream: &mut TcpStream, started_at: Instant) {
	let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
	let mut buffer = [0_u8; 1024];
	let Ok(size) = stream.read(&mut buffer) else { return; };
	let request = String::from_utf8_lossy(&buffer[..size]);
	let path = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).unwrap_or("/");
	if path != "/health" && path != "/api/health" {
		write_response(stream, "404 Not Found", r#"{"status":"not_found"}"#);
		return;
	}
	let body = format!(r#"{{"status":"ok","service":"game-server","uptimeSecs":{}}}"#, started_at.elapsed().as_secs());
	write_response(stream, "200 OK", &body);
}

fn write_response(stream: &mut TcpStream, status: &str, body: &str) {
	let response = format!(
		"HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
		status,
		body.as_bytes().len(),
		body
	);
	let _ = stream.write_all(response.as_bytes());
}
