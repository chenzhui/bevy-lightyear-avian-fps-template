use core::net::{Ipv4Addr, SocketAddr};
use std::net::IpAddr;
use std::time::Duration;
use bevy::prelude::*;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use lightyear::netcode::{ConnectToken, NetcodeClient, NetcodeServer};
use lightyear::prelude::server::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use crate::vleue::util::env::{env_string, env_u16, env_u64};

pub const FIXED_TIMESTEP_HZ: f64 = 64.0;/// Fixed timestep frequency in Hz, used for game loop updates
pub const CLIENT_PORT: u16 = 5887;/// Client port number, 0 means OS will assign any available port
pub const SEND_INTERVAL: Duration = Duration::from_millis(100);/// Send interval, used to control packet send frequency

pub const STEAM_APP_ID: u32 = 480; // Steamworks App ID for Spacewar, used for testing

#[derive(Copy, Clone, Debug)]
pub struct SharedSettings {/// Shared settings struct, defining network protocol parameters
	pub protocol_id: u64,	/// Protocol identifier
	pub private_key: [u8; 32], // Private key for security verification
}

pub fn shared_settings() -> SharedSettings {
	SharedSettings {
		protocol_id: env_u64("GAME_NETCODE_PROTOCOL_ID", 0),
		private_key: netcode_private_key(),
	}
}

fn netcode_private_key() -> [u8; 32] {
	let Ok(value) = std::env::var("GAME_NETCODE_PRIVATE_KEY_HEX") else {
		warn!("GAME_NETCODE_PRIVATE_KEY_HEX is not set; using the demo-only zero netcode key");
		return [0; 32];
	};
	match parse_hex_key_32(&value) {
		Some(key) => key,
		None => {
			warn!("GAME_NETCODE_PRIVATE_KEY_HEX must be exactly 64 hex characters; using the demo-only zero netcode key");
			[0; 32]
		}
	}
}

fn parse_hex_key_32(value: &str) -> Option<[u8; 32]> {
	let hex = value.trim();
	if hex.len() != 64 {
		return None;
	}
	let mut key = [0u8; 32];
	for index in 0..32 {
		let start = index * 2;
		key[index] = u8::from_str_radix(&hex[start..start + 2], 16).ok()?;
	}
	Some(key)
}

pub fn server_port() -> u16 {
	env_u16("GAME_SERVER_PORT", 5888)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ServerTransports {
	Udp { local_port: u16 },
}



//region server

#[derive(Component, Debug)]
#[component(on_add = ServerConnection::on_add)]  // When this component is added to an entity, automatically call ExampleServer::on_add as callback
pub struct ServerConnection {
	pub conditioner: Option<RecvLinkConditioner>,
	pub transport: ServerTransports,
	pub shared: SharedSettings,
}

impl ServerConnection {
	fn on_add(mut world: DeferredWorld, context: HookContext) {
		let entity = context.entity;
		world.commands().queue(move |world: &mut World| -> Result {
			let mut entity_mut = world.entity_mut(entity);
			let settings = entity_mut.take::<ServerConnection>().unwrap();
			entity_mut.insert((Name::from("Server"),));

			let add_netcode = |entity_mut: &mut EntityWorldMut| {
				entity_mut.insert(NetcodeServer::new(NetcodeConfig {
					protocol_id: settings.shared.protocol_id,
					private_key: settings.shared.private_key,
					..Default::default()
				}));
			};
			match settings.transport {
				ServerTransports::Udp { local_port } => {
					add_netcode(&mut entity_mut);
					let server_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), local_port);
					entity_mut.insert((LocalAddr(server_addr), ServerUdpIo::default()));
				}
				_ => {}
			};
			Ok(())
		});
	}
}

pub(crate) fn why_start(mut commands: Commands, server: Single<Entity, With<Server>>) {
	commands.trigger(Start {
		entity: server.into_inner(),
	});
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WebTransportCertificateSettings {
	AutoSelfSigned(Vec<String>),
}

impl Default for WebTransportCertificateSettings {
	fn default() -> Self {
		WebTransportCertificateSettings::AutoSelfSigned(vec!["localhost".to_string(), "127.0.0.1".to_string()])
	}
}

// impl From<&WebTransportCertificateSettings> for Identity {
//     fn from(wt: &WebTransportCertificateSettings) -> Identity {
//         match wt {
//             WebTransportCertificateSettings::AutoSelfSigned(sans) => {
//                 Identity::self_signed(sans).unwrap()
//             }
//         }
//     }
// }


//endregion server


//region client


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClientTransports {
	Udp,
	WebTransport,
	WebSocket,
}

#[derive(Component, Clone, Debug)]
#[component(on_add = ClientConnection::on_add)]
pub struct ClientConnection {
	pub netcode_client_id: u64,
	pub client_port: u16,
	pub server_addr: SocketAddr,
	pub conditioner: Option<RecvLinkConditioner>,
	pub transport: ClientTransports,
	pub shared: SharedSettings,
}

impl ClientConnection {
	fn on_add(mut world: DeferredWorld, context: HookContext) {
		debug!("ClientConnection::on_add");
		let entity = context.entity;
		world.commands().queue(move |world: &mut World| -> Result {
			let mut entity_mut = world.entity_mut(entity);
			let settings = entity_mut.take::<ClientConnection>().unwrap();
			let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), settings.client_port);
			entity_mut.insert((
				Client::default(),
				Link::new(settings.conditioner.clone()),
				LocalAddr(client_addr),
				PeerAddr(settings.server_addr),
				ReplicationReceiver::default(),
				PredictionManager::default(),
				Name::from("Client"),
			));

			let add_netcode = |entity_mut: &mut EntityWorldMut| -> Result {
				let token = ConnectToken::build(
					settings.server_addr,
					settings.shared.protocol_id,
					settings.netcode_client_id,
					settings.shared.private_key,
				)
				.generate()?;
				let auth = Authentication::Token(token);
				let netcode_config = lightyear::netcode::client_plugin::NetcodeConfig {
					client_timeout_secs: 10,
					token_expire_secs: -1,
					..default()
				};
				entity_mut.insert(NetcodeClient::new(auth, netcode_config)?);
				Ok(())
			};

			match settings.transport {
				ClientTransports::Udp => {
					add_netcode(&mut entity_mut)?;
					entity_mut.insert(UdpIo::default());
				}
				// ClientTransports::WebTransport => {
				//     add_netcode(&mut entity_mut)?;
				//     entity_mut.insert(WebTransportClientIo { certificate_digest: "".to_string() });
				// }
				// ClientTransports::WebSocket => {
				//     add_netcode(&mut entity_mut)?;
				//     entity_mut.insert(WebSocketClientIo {
				//         config: ClientConfig::default(),
				//         target: WebSocketTarget::Addr(Default::default()),
				//     });
				// }
				_ => {}
			};
			Ok(())
		});
	}
}

pub(crate) fn connect(mut commands: Commands, client: Single<Entity, With<Client>>) {
	commands.trigger(Connect {
		entity: client.into_inner(),
	});
}

//endregion client
