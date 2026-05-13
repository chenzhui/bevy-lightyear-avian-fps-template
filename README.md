# Bevy Lightyear FPS Example

A high-performance, multiplayer 3D FPS prototype demonstrating advanced networking patterns with [Bevy](https://bevyengine.org/) and [Lightyear](https://github.com/cBournhonesque/lightyear).

## Project Overview

This project serves as a reference architecture for building networked, room-based multiplayer games in Rust. It highlights key patterns for scalable game backends and synchronized client-side simulation.

### Core Features

- **Networking:** Full client-side prediction, interpolation, and server-authoritative state synchronization using Lightyear.
- **Matchmaking:** Integrated lobby system for game session management.
- **Room-Based Architecture:** Scalable game sessions with entry validation and authentication.
- **Dynamic World:** Chunk-based map streaming and server-authoritative entity management.
- **Developer Tools:** Built-in debug camera, performance monitoring, and server-side testing hooks.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- Java 21+ (for the `room-service` backend)
- Maven (for building the backend)

### Configuration

1. **Environment Setup**: Copy the example configuration file:
   ```bash
   cp .env.example .env
   ```

2. **Configure Variables**: Edit `.env` with your local or remote server settings:
   - `WZH_GAME_BACKEND_BASE_URL`: API base URL for lobby/matchmaking.
   - `WZH_GAME_SERVER_HOST`: Game server network address.
   - `WZH_GAME_SERVER_PORT`: Game server communication port.
   - `WZH_GAME_HEALTH_PORT`: Monitoring port.

*Note: `.env` is ignored by git to protect your secrets.*

### Running the Project

#### 1. Start the Room Service Backend
Navigate to the `room-service` directory and start the Spring Boot application:
```bash
cd room-service
mvn spring-boot:run
```

#### 2. Run the Game Server
In the project root, start the game server:
```bash
cargo run -- server
```

#### 3. Run the Game Client
In the project root, start the game client:
```bash
cargo run -- client
```

## Architecture

- **`src/`**: Rust-based game client and server logic.
- **`room-service/`**: Java/Spring Boot backend for matchmaking and session orchestration.
- **`assets/`**: Game assets and collision maps.

## Disclaimer

This project is an architectural prototype for network synchronization patterns and should be treated as a starting point for development, not a production-ready game.

## License

This project is licensed under the [MIT License](../LICENSE).
