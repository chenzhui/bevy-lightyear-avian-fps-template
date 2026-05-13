use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_symbios_ground::HeightMapMeshBuilder;
use lightyear_replication::prelude::{Room, RoomEvent, RoomTarget};
use once_cell::sync::Lazy;
use serde::Deserialize;
use symbios_ground::HeightMap;

use crate::vleue::feature::character::movement::{PLAYER_LAYER, WORLD_LAYER};
use crate::vleue::feature::core::connection::GameRoomId;
use crate::vleue::feature::core::room::MatchRoomStates;
use crate::vleue::feature::core::state::{InGameState, RoomPhaseState};
use crate::vleue::feature::VleueSide;

pub mod client;

pub const MAP_COLLISION_PATH: &str = "map_collision.ron";
pub const MAP_VISUAL_Y: f32 = 0.0;
pub const MAP_COLLIDER_MODE_CUBOID: bool = false; // Use stable cuboid first to verify physics issues, later switch back to height field.
pub const FALLBACK_FLOOR_HALF_WIDTH: f32 = 150.0; // Terrain fallback collision width, wider than 246m heightfield to avoid falling off edges.
pub const FALLBACK_FLOOR_HALF_HEIGHT: f32 = 0.5; // Thick floor half height, gives Avian a stable cuboid contact surface.
pub const FALLBACK_FLOOR_Y: f32 = -1.2; // Top surface around -0.7, close to current heightfield lowest point below.
pub const TERRAIN_CHUNK_CELL_COUNT: usize = 16; // 65x65 heightmap becomes 4x4 tiles cleanly.
pub const STREAMING_FALLBACK_CHUNK_SIZE: f32 = 64.0; // Fallback chunk size when no terrain exists.

pub static MAP_COLLISION_CONFIG: Lazy<MapCollisionConfig> = Lazy::new(load_map_collision_config);

#[derive(Component)]
pub struct MapLogicMarker; // Server room map logic marker, avoids duplicate generation.

#[derive(Component)]
pub struct MapColliderMarker; // Local map collider marker; server/client generate separately, not network synced.

#[derive(Clone, Debug, Deserialize)]
pub struct MapCollisionConfig {
    pub terrain: Option<MapTerrainHeightfieldConfig>,
    #[serde(default)]pub cuboids: Vec<MapCuboidColliderConfig>,
    #[serde(default)]pub glb_instances: Vec<MapGlbInstanceConfig>, // Reserved for future "split GLB + RON splice" asset pipeline.
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapTerrainHeightfieldConfig {
    pub name: String,
    pub rows: usize,
    pub columns: usize,
    pub scale: Vec<f32>,
    pub heights: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapCuboidColliderConfig {
    pub name: String,
    pub translation: Vec<f32>,
    pub size: Vec<f32>,
    #[serde(default)]pub rotation_y: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MapGlbInstanceConfig {
    pub name: String,
    pub glb_path: String, #[serde(default)]
    pub scene: usize,
    pub translation: Vec<f32>,
    #[serde(default = "default_instance_scale")]pub scale: Vec<f32>,
    #[serde(default)]pub rotation_y: f32,
    #[serde(default)]pub collider: Option<MapCuboidColliderConfig>,
}

fn default_instance_scale() -> Vec<f32> { vec![1.0, 1.0, 1.0] }

pub struct MapPlugin {
    pub side: VleueSide,
    pub headless: bool,
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MapShaderPlugin);
        if self.side.is_client() {
            app.add_plugins(client::MapClientPlugin);
        } else if self.side.is_server() {
            app.add_plugins(MapServerPlugin { headless: self.headless });
        }
    }
}

pub struct MapShaderPlugin;

impl Plugin for MapShaderPlugin {
    fn build(&self, _app: &mut App) {}
}

pub struct MapServerPlugin {
    pub headless: bool,
}

impl Plugin for MapServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_waiting_room_maps);
        app.add_systems(OnEnter(InGameState::Loading), spawn_server_map_visuals);
    }
}

fn spawn_waiting_room_maps(mut commands: Commands, mut room_states: ResMut<MatchRoomStates>, rooms: Query<(Entity, &GameRoomId), With<Room>>, maps: Query<&GameRoomId, With<MapLogicMarker>>,) {
    for (room_entity, room_id) in &rooms {
        let Some(room_state) = room_states.rooms.get(&room_id.0) else { continue; };
        if room_state.phase != RoomPhaseState::Waiting || room_state.map_loaded || maps.iter().any(|map_room| map_room.0 == room_id.0) {
            continue;
        }

        if commands.get_entity(room_entity).is_err() {
            continue;
        }

        let map_root = commands.spawn((
            Name::new(format!("MapLogic_{}", room_id.0)),
            MapLogicMarker,
            GameRoomId(room_id.0),
        )).id();
        commands.trigger(RoomEvent {
            target: RoomTarget::AddEntity(map_root),
            room: room_entity,
        });
        for map_entity in spawn_map_collision_entities(&mut commands, Some(room_id.0), &format!("Room{}_MapCollision", room_id.0), true) {
            commands.trigger(RoomEvent {
                target: RoomTarget::AddEntity(map_entity),
                room: room_entity,
            });
        }
        room_states.mark_map_loaded(room_id.0);
        info!("[server] room {} map logic spawned root={:?}", room_id.0, map_root);
    }
}

fn spawn_server_map_visuals(mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    info!("[map] server map loading");
    spawn_map_terrain_visual(&mut commands, &mut meshes, &mut materials);
    spawn_map_glb_instances(&mut commands, &asset_server, "ServerMapPart");
    spawn_map_collision_entities(&mut commands, None, "ServerMapCollision", true);
}

pub fn load_map_collision_config() -> MapCollisionConfig {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest_dir.join("assets").join(MAP_COLLISION_PATH),
        manifest_dir.join("assets").join(MAP_COLLISION_PATH),
        std::env::current_dir().unwrap_or_default().join("assets").join(MAP_COLLISION_PATH),
    ];
    let path = candidates.iter().find(|path| path.exists()).cloned().unwrap_or_else(|| candidates[0].clone());
    let content = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let config: MapCollisionConfig = ron::from_str(&content).unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));
    if let Some(terrain) = &config.terrain {
        assert_eq!(terrain.rows, terrain.heights.len(), "map heightfield row count mismatch");
        assert_eq!(terrain.scale.len(), 3, "map heightfield scale must have 3 values");
        assert!(terrain.heights.iter().all(|row| row.len() == terrain.columns), "map heightfield column count mismatch");
    }
    for cuboid in &config.cuboids {
        assert_eq!(cuboid.translation.len(), 3, "map cuboid {} translation must have 3 values", cuboid.name);
        assert_eq!(cuboid.size.len(), 3, "map cuboid {} size must have 3 values", cuboid.name);
    }
    for instance in &config.glb_instances {
        assert_eq!(instance.translation.len(), 3, "map glb instance {} translation must have 3 values", instance.name);
        assert_eq!(instance.scale.len(), 3, "map glb instance {} scale must have 3 values", instance.name);
        if let Some(collider) = &instance.collider {
            assert_eq!(collider.translation.len(), 3, "map glb instance {} collider translation must have 3 values", instance.name);
            assert_eq!(collider.size.len(), 3, "map glb instance {} collider size must have 3 values", instance.name);
        }
    }
    config
}

pub fn spawn_map_collision_entities(commands: &mut Commands, room_id: Option<u64>, name_prefix: &str, include_instance_colliders: bool) -> Vec<Entity> {
    let config = &*MAP_COLLISION_CONFIG;
    let mut entities = Vec::new();
    entities.push(spawn_fallback_floor_collider(commands, room_id, name_prefix));
    if let Some(terrain) = &config.terrain {
        let mut entity = commands.spawn((
            Name::new(format!("{}_{}", name_prefix, terrain.name)),
            MapColliderMarker,
            RigidBody::Static,
            Collider::heightfield(terrain.heights.clone(), vec3_from_slice(&terrain.scale)),
            CollisionLayers::new(WORLD_LAYER, PLAYER_LAYER),
            Transform::from_xyz(0.0, MAP_VISUAL_Y, 0.0),
        ));
        if let Some(room_id) = room_id {
            entity.insert(GameRoomId(room_id));
        }
        entities.push(entity.id());
    }
    for cuboid in &config.cuboids {
        entities.push(spawn_cuboid_collider(commands, room_id, name_prefix, cuboid, Vec3::ZERO, 0.0));
    }
    if include_instance_colliders {
        for instance in &config.glb_instances {
            if let Some(collider) = &instance.collider {
                entities.push(spawn_cuboid_collider(commands, room_id, &format!("{}_{}", name_prefix, instance.name), collider, vec3_from_slice(&instance.translation), instance.rotation_y));
            }
        }
    }
    entities
}

fn spawn_fallback_floor_collider(commands: &mut Commands, room_id: Option<u64>, name_prefix: &str) -> Entity {
    let mut entity = commands.spawn((
        Name::new(format!("{}_FallbackFloor", name_prefix)),
        MapColliderMarker,
        RigidBody::Static,
        Collider::cuboid(FALLBACK_FLOOR_HALF_WIDTH, FALLBACK_FLOOR_HALF_HEIGHT, FALLBACK_FLOOR_HALF_WIDTH),
        CollisionLayers::new(WORLD_LAYER, PLAYER_LAYER),
        Transform::from_xyz(0.0, FALLBACK_FLOOR_Y, 0.0),
    ));
    if let Some(room_id) = room_id {
        entity.insert(GameRoomId(room_id));
    }
    entity.id()
}

fn spawn_map_glb_instances(commands: &mut Commands, asset_server: &AssetServer, name_prefix: &str) {
    for instance in &MAP_COLLISION_CONFIG.glb_instances {
        commands.spawn((
            Name::new(format!("{}_{}", name_prefix, instance.name)),
            SceneRoot(asset_server.load(GltfAssetLabel::Scene(instance.scene).from_asset(instance.glb_path.clone()))),
            Transform::from_translation(vec3_from_slice(&instance.translation)).with_rotation(Quat::from_rotation_y(instance.rotation_y)).with_scale(vec3_from_slice(&instance.scale)),
        ));
    }
}

pub fn spawn_map_terrain_visual(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
    let Some(terrain) = &MAP_COLLISION_CONFIG.terrain else { return; };
    let heightmap = build_terrain_heightmap(terrain);
    let mesh = HeightMapMeshBuilder::new().with_uv_tile_size(8.0).build(&heightmap);
    let terrain_width = terrain_axis_length(terrain.rows, terrain.scale[0]);
    let terrain_depth = terrain_axis_length(terrain.columns, terrain.scale[2]);
    commands.spawn((
        Name::new(format!("MapTerrainVisual_{}", terrain.name)),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.42, 0.22),
            perceptual_roughness: 0.9,
            ..default()
        })),
        Transform::from_xyz(-terrain_width * 0.5, MAP_VISUAL_Y, -terrain_depth * 0.5),
    ));
}


pub fn build_terrain_heightmap(terrain: &MapTerrainHeightfieldConfig) -> HeightMap {
    let cell_size_x = terrain_cell_size(terrain.rows, terrain.scale[0]);
    let cell_size_z = terrain_cell_size(terrain.columns, terrain.scale[2]);
    debug_assert!((cell_size_x - cell_size_z).abs() < f32::EPSILON, "terrain visual expects uniform x/z cell size");
    let mut heightmap = HeightMap::new(terrain.rows, terrain.columns, cell_size_x);
    for x in 0..terrain.rows {
        for z in 0..terrain.columns {
            heightmap.set(x, z, terrain.heights[x][z] * terrain.scale[1]);
        }
    }
    heightmap
}


fn terrain_cell_size(points: usize, size: f32) -> f32 {
    size / (points.saturating_sub(1).max(1) as f32)
}

fn terrain_axis_length(points: usize, size: f32) -> f32 {
    terrain_cell_size(points, size) * points.saturating_sub(1) as f32
}
pub fn vec3_from_slice(values: &[f32]) -> Vec3 { Vec3::new(values[0], values[1], values[2]) }



fn spawn_cuboid_collider(commands: &mut Commands, room_id: Option<u64>, name_prefix: &str, cuboid: &MapCuboidColliderConfig, parent_translation: Vec3, parent_rotation_y: f32) -> Entity {
    let local_translation = vec3_from_slice(&cuboid.translation);
    let rotation_y = parent_rotation_y + cuboid.rotation_y;
    let mut entity = commands.spawn((
        Name::new(format!("{}_{}", name_prefix, cuboid.name)),
        MapColliderMarker,
        RigidBody::Static,
        Collider::cuboid(cuboid.size[0], cuboid.size[1], cuboid.size[2]),
        CollisionLayers::new(WORLD_LAYER, PLAYER_LAYER),
        Transform::from_translation(parent_translation + Quat::from_rotation_y(parent_rotation_y) * local_translation).with_rotation(Quat::from_rotation_y(rotation_y)),
    ));
    if let Some(room_id) = room_id {
        entity.insert(GameRoomId(room_id));
    }
    entity.id()
}
