use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use avian3d::prelude::*;
use avian3d::physics_transform::Position;
use lightyear::prelude::Predicted;
use lightyear_replication::prelude::Controlled;
use crate::vleue::feature::character::movement::{CharacterMarker, WORLD_LAYER, PLAYER_LAYER};
use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::core::state::InGameState;
use crate::vleue::feature::level::map::{self, MapCollisionConfig, MapTerrainHeightfieldConfig};
use bevy_symbios_ground::HeightMapMeshBuilder;
use symbios_ground::HeightMap;

#[derive(Resource)]
pub struct MapStreamingRuntime {
    pub terrain_material: Handle<StandardMaterial>, // Material used for terrain chunks
    pub catalog: MapStreamingCatalog,              // Map streaming data catalog
    pub loaded_chunks: HashMap<MapChunkKey, Entity>, // Currently loaded map chunks
}

#[derive(Default)]
pub struct MapStreamingCatalog {
    pub chunk_size: Vec2,                          // Size of each map chunk
    pub chunks: HashMap<MapChunkKey, MapChunkSpec>, // All available map chunk specifications
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MapChunkKey {
    pub x: i32, // Chunk coordinate X
    pub z: i32, // Chunk coordinate Z
}

pub struct MapChunkSpec {
    pub origin: Vec3,                          // World origin of this chunk
    pub center: Vec3,                          // World center of this chunk
    pub half_size: Vec2,                       // Half size for distance calculation
    pub terrain: Option<MapTerrainChunkSpec>,  // Terrain data if present
    pub glb_instance_indices: Vec<usize>,      // Indices of GLB instances in this chunk
}

pub struct MapTerrainChunkSpec {
    pub rows_start: usize,    // Starting row index in terrain data
    pub rows: usize,          // Number of rows in this chunk
    pub columns_start: usize, // Starting column index in terrain data
    pub columns: usize,       // Number of columns in this chunk
}

pub const STREAMING_LOAD_RADIUS: f32 = 96.0; // Chunk load radius in world units.
pub const STREAMING_UNLOAD_RADIUS: f32 = 120.0; // Chunk unload radius in world units, slightly larger than load radius to avoid thrashing.

#[derive(Component)]
pub struct MapChunkMarker; // Marker for map streaming chunks

pub struct MapClientPlugin;

impl Plugin for MapClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(InGameState::Loading), setup_client_map_streaming);
        app.add_systems(Update, update_client_map_streaming);
        app.add_systems(OnExit(InGameState::Playing), cleanup_client_map_streaming);
    }
}

pub(crate) fn setup_client_map_streaming(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) { // Initialize the client streaming runtime and material cache.
    info!("[map] setup client streaming runtime");
    commands.remove_resource::<MapStreamingRuntime>(); // make sure the resource is cleaned
    let catalog = build_map_streaming_catalog(&map::MAP_COLLISION_CONFIG);
    let terrain_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.42, 0.22),
        perceptual_roughness: 0.9,
        ..default()
    });
    map::spawn_map_collision_entities(&mut commands, None, "ClientMapCollision", false);
    commands.insert_resource(MapStreamingRuntime {
        terrain_material,
        catalog,
        loaded_chunks: HashMap::new(),
    });
}

pub(crate) fn update_client_map_streaming(state: Option<Res<State<InGameState>>>, player_query: Query<&Position, (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>)>,
    mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, runtime: Option<ResMut<MapStreamingRuntime>>,
) { // Load and unload map chunks based on player distance.
    let Some(state) = state else { return; };
    if !matches!(state.get(), InGameState::Loading | InGameState::Playing) {
        return;
    }
    let Some(mut runtime) = runtime else { return; };
    let Some(player_position) = player_query.iter().next() else { return; };
    let player_pos = player_position.0;

    let mut desired_chunks = HashSet::new();
    for (chunk_key, chunk_spec) in &runtime.catalog.chunks {
        if distance_point_to_chunk(player_pos, chunk_spec) <= STREAMING_LOAD_RADIUS {
            desired_chunks.insert(*chunk_key);
        }
    }

    let loaded_keys: Vec<MapChunkKey> = runtime.loaded_chunks.keys().copied().collect();
    for chunk_key in loaded_keys {
        let Some(chunk_spec) = runtime.catalog.chunks.get(&chunk_key) else { continue; };
        let distance = distance_point_to_chunk(player_pos, chunk_spec);
        if distance > STREAMING_UNLOAD_RADIUS && !desired_chunks.contains(&chunk_key) {
            if let Some(chunk_root) = runtime.loaded_chunks.remove(&chunk_key) {
                commands.entity(chunk_root).despawn();
            }
        }
    }

    for chunk_key in desired_chunks {
        if runtime.loaded_chunks.contains_key(&chunk_key) {
            continue;
        }
        let Some(chunk_spec) = runtime.catalog.chunks.get(&chunk_key) else { continue; };
        let chunk_root = spawn_map_chunk(&mut commands, &asset_server, &mut meshes, &runtime.terrain_material, &map::MAP_COLLISION_CONFIG, chunk_key, chunk_spec, );
        runtime.loaded_chunks.insert(chunk_key, chunk_root);
    }
}

pub(crate) fn cleanup_client_map_streaming(mut commands: Commands, runtime: Option<Res<MapStreamingRuntime>>) { // Remove all streamed chunk entities when leaving gameplay.
    let Some(runtime) = runtime else { return; };
    for chunk_root in runtime.loaded_chunks.values().copied() {
        commands.entity(chunk_root).despawn();
    }
    commands.remove_resource::<MapStreamingRuntime>();
    info!("[map] cleaned up client map streaming");
}

pub(crate) fn spawn_map_chunk(commands: &mut Commands, asset_server: &AssetServer, meshes: &mut Assets<Mesh>, terrain_material: &Handle<StandardMaterial>, config: &MapCollisionConfig, chunk_key: MapChunkKey, chunk_spec: &MapChunkSpec, ) -> Entity { // Spawn one streamed terrain chunk and its attached instances.
    let chunk_root = commands.spawn((
        Name::new(format!("MapChunk_{}_{}", chunk_key.x, chunk_key.z)),
        MapChunkMarker,
        Transform::from_xyz(chunk_spec.origin.x, map::MAP_VISUAL_Y, chunk_spec.origin.z),
    )).id();
    commands.entity(chunk_root).with_children(|parent| {
        if let Some(terrain_chunk) = &chunk_spec.terrain {
            if let Some(terrain) = &config.terrain {
                let mesh = build_terrain_chunk_mesh(terrain, terrain_chunk);
                parent.spawn((
                    Name::new(format!("MapChunkTerrain_{}_{}", chunk_key.x, chunk_key.z)),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(terrain_material.clone()),
                    Transform::default(),
                ));
            }
        }
        for &instance_index in &chunk_spec.glb_instance_indices {
            let instance = &config.glb_instances[instance_index];
            let local_translation = map::vec3_from_slice(&instance.translation) - chunk_spec.origin;
            parent.spawn((
                Name::new(format!("MapChunkInstance_{}_{}_{}", chunk_key.x, chunk_key.z, instance.name)),
                SceneRoot(asset_server.load(GltfAssetLabel::Scene(instance.scene).from_asset(instance.glb_path.clone()))),
                Transform::from_translation(local_translation).with_rotation(Quat::from_rotation_y(instance.rotation_y)).with_scale(map::vec3_from_slice(&instance.scale)),
            ));
            if let Some(collider) = &instance.collider {
                let collider_local_translation = local_translation + Quat::from_rotation_y(instance.rotation_y) * map::vec3_from_slice(&collider.translation);
                parent.spawn((
                    Name::new(format!("MapChunkInstanceCollider_{}_{}_{}", chunk_key.x, chunk_key.z, instance.name)),
                    map::MapColliderMarker,
                    RigidBody::Static,
                    Collider::cuboid(collider.size[0], collider.size[1], collider.size[2]),
                    CollisionLayers::new(WORLD_LAYER, PLAYER_LAYER),
                    Transform::from_translation(collider_local_translation).with_rotation(Quat::from_rotation_y(instance.rotation_y + collider.rotation_y)),
                ));
            }
        }
    });
    chunk_root
}

fn build_terrain_chunk_mesh(terrain: &MapTerrainHeightfieldConfig, chunk: &MapTerrainChunkSpec) -> Mesh { // Build a chunk-local terrain mesh from the source heightmap.
    let cell_size_x = map::terrain_cell_size(terrain.rows, terrain.scale[0]);
    let cell_size_z = map::terrain_cell_size(terrain.columns, terrain.scale[2]);
    debug_assert!((cell_size_x - cell_size_z).abs() < f32::EPSILON, "terrain visual expects uniform x/z cell size");
    let mut heightmap = HeightMap::new(chunk.rows, chunk.columns, cell_size_x);
    for row in 0..chunk.rows {
        for column in 0..chunk.columns {
            heightmap.set(row, column, terrain.heights[chunk.rows_start + row][chunk.columns_start + column] * terrain.scale[1]);
        }
    }
    HeightMapMeshBuilder::new().with_uv_tile_size(8.0).build(&heightmap)
}

pub(crate) fn build_map_streaming_catalog(config: &MapCollisionConfig) -> MapStreamingCatalog { // Precompute chunk boundaries and chunk ownership for all instances.
    let mut catalog = MapStreamingCatalog::default();
    let (chunk_size, terrain_origin) = if let Some(terrain) = &config.terrain {
        let total_cells_x = terrain.rows.saturating_sub(1);
        let total_cells_z = terrain.columns.saturating_sub(1);
        let chunk_cells_x = map::TERRAIN_CHUNK_CELL_COUNT.min(total_cells_x.max(1));
        let chunk_cells_z = map::TERRAIN_CHUNK_CELL_COUNT.min(total_cells_z.max(1));
        let cell_size_x = map::terrain_cell_size(terrain.rows, terrain.scale[0]);
        let cell_size_z = map::terrain_cell_size(terrain.columns, terrain.scale[2]);
        let chunk_size = Vec2::new(chunk_cells_x as f32 * cell_size_x, chunk_cells_z as f32 * cell_size_z);
        let terrain_origin = Vec3::new(-map::terrain_axis_length(terrain.rows, terrain.scale[0]) * 0.5, 0.0, -map::terrain_axis_length(terrain.columns, terrain.scale[2]) * 0.5, );
        let chunk_count_x = total_cells_x.div_ceil(chunk_cells_x);
        let chunk_count_z = total_cells_z.div_ceil(chunk_cells_z);

        for chunk_x in 0..chunk_count_x {
            for chunk_z in 0..chunk_count_z {
                let rows_start = chunk_x * chunk_cells_x;
                let columns_start = chunk_z * chunk_cells_z;
                let rows = ((rows_start + chunk_cells_x).min(terrain.rows.saturating_sub(1)) - rows_start) + 1;
                let columns = ((columns_start + chunk_cells_z).min(terrain.columns.saturating_sub(1)) - columns_start) + 1;
                let origin = terrain_origin + Vec3::new(rows_start as f32 * cell_size_x, 0.0, columns_start as f32 * cell_size_z);
                let half_size = Vec2::new((rows.saturating_sub(1)) as f32 * cell_size_x * 0.5, (columns.saturating_sub(1)) as f32 * cell_size_z * 0.5);
                let center = origin + Vec3::new(half_size.x, 0.0, half_size.y);
                let key = MapChunkKey { x: chunk_x as i32, z: chunk_z as i32 };
                catalog.chunks.insert(key, MapChunkSpec {
                    origin,
                    center,
                    half_size,
                    terrain: Some(MapTerrainChunkSpec {
                        rows_start,
                        rows,
                        columns_start,
                        columns,
                    }),
                    glb_instance_indices: Vec::new(),
                });
            }
        }
        (chunk_size, terrain_origin)
    } else {
        (Vec2::splat(map::STREAMING_FALLBACK_CHUNK_SIZE), Vec3::ZERO)
    };

    for (instance_index, instance) in config.glb_instances.iter().enumerate() {
        let chunk_key = chunk_key_for_position(map::vec3_from_slice(&instance.translation), terrain_origin, chunk_size).unwrap_or(MapChunkKey { x: 0, z: 0 });
        let entry = catalog.chunks.entry(chunk_key).or_insert_with(|| {
            let origin = terrain_origin + Vec3::new(chunk_key.x as f32 * chunk_size.x, 0.0, chunk_key.z as f32 * chunk_size.y);
            let center = origin + Vec3::new(chunk_size.x * 0.5, 0.0, chunk_size.y * 0.5);
            MapChunkSpec {
                origin,
                center,
                half_size: chunk_size * 0.5,
                terrain: None,
                glb_instance_indices: Vec::new(),
            }
        });
        entry.glb_instance_indices.push(instance_index);
    }
    catalog.chunk_size = chunk_size;
    catalog
}

fn chunk_key_for_position(position: Vec3, origin: Vec3, chunk_size: Vec2) -> Option<MapChunkKey> { // Convert a world position into a chunk coordinate.
    if chunk_size.x <= 0.0 || chunk_size.y <= 0.0 {
        return None;
    }
    let local = position - origin;
    Some(MapChunkKey {
        x: (local.x / chunk_size.x).floor() as i32,
        z: (local.z / chunk_size.y).floor() as i32,
    })
}

pub(crate) fn distance_point_to_chunk(position: Vec3, chunk: &MapChunkSpec) -> f32 { // Measure planar distance from a point to the chunk bounds.
    let dx = (position.x - chunk.center.x).abs() - chunk.half_size.x;
    let dz = (position.z - chunk.center.z).abs() - chunk.half_size.y;
    let clamped_dx = dx.max(0.0);
    let clamped_dz = dz.max(0.0);
    (clamped_dx * clamped_dx + clamped_dz * clamped_dz).sqrt()
}
