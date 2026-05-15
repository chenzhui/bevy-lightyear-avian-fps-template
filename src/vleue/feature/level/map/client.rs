use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::character::movement::{CharacterMarker, PLAYER_LAYER, WORLD_LAYER};
use crate::vleue::feature::core::state::InGameState;
use crate::vleue::feature::level::map::{self, MapCollisionConfig, MapTerrainHeightfieldConfig};
use avian3d::physics_transform::Position;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use bevy_symbios_ground::HeightMapMeshBuilder;
use futures_lite::future;
use lightyear::prelude::Predicted;
use lightyear_replication::prelude::Controlled;
use std::collections::{HashMap, HashSet};
use symbios_ground::HeightMap;

#[derive(Resource)]
pub struct MapStreamingRuntime { // Runtime state owned by the client-side map streamer.
    pub terrain_material: Handle<StandardMaterial>, // Shared material used by all streamed terrain chunk meshes.
    pub catalog: MapStreamingCatalog, // Precomputed static chunk metadata derived from the map collision config.
    pub loaded_chunks: HashMap<MapChunkKey, Entity>, // Chunk root entities that are currently spawned in the world.
    pub loading_chunks: HashMap<MapChunkKey, Task<MapChunkBuildResult>>, // Background terrain mesh builds that have been requested but not spawned yet.
}

#[derive(Default)]
pub struct MapStreamingCatalog { // Static lookup table used to decide which chunk owns terrain and GLB instances.
    pub chunk_size: Vec2, // Nominal chunk size on the horizontal X/Z plane, in world units.
    pub chunks: HashMap<MapChunkKey, MapChunkSpec>, // All known chunks keyed by integer chunk coordinates.
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct MapChunkKey { // Integer coordinate of a streamed map chunk on the X/Z plane.
    pub x: i32, // Chunk coordinate along world X.
    pub z: i32, // Chunk coordinate along world Z.
}

pub struct MapChunkSpec { // Precomputed spawn data for one map chunk.
    pub origin: Vec3, // Minimum world-space corner of this chunk on the X/Z plane.
    pub center: Vec3, // World-space center used for distance checks and load prioritization.
    pub half_size: Vec2, // Half extents on X/Z used to measure distance to the chunk bounds.
    pub terrain: Option<MapTerrainChunkSpec>, // Source heightmap window for this chunk, when terrain exists.
    pub glb_instance_indices: Vec<usize>, // Indices into MapCollisionConfig::glb_instances owned by this chunk.
}

#[derive(Clone)]
pub struct MapTerrainChunkSpec { // Rectangular window into the full terrain heightmap for one chunk mesh.
    pub rows_start: usize, // First source row in the full heightmap.
    pub rows: usize, // Number of heightmap rows copied into this chunk mesh.
    pub columns_start: usize, // First source column in the full heightmap.
    pub columns: usize, // Number of heightmap columns copied into this chunk mesh.
}

pub const STREAMING_LOAD_RADIUS: f32 = 96.0; // Chunk load radius in world units.
pub const STREAMING_UNLOAD_RADIUS: f32 = 120.0; // Chunk unload radius in world units, slightly larger than load radius to avoid thrashing.
pub const STREAMING_MAX_NEW_LOADS_PER_FRAME: usize = 2; // Limit chunk build bursts to avoid frame spikes.
pub const STREAMING_MAX_SPAWNS_PER_FRAME: usize = 2; // Limit entity creation bursts after async mesh builds finish.
pub const STREAMING_MAX_ASYNC_BUILDS: usize = 8; // Same idea as Ethertum: cap concurrent chunk work.

pub struct MapChunkBuildResult { // Output produced by the background terrain mesh builder.
    pub chunk_key: MapChunkKey, // Chunk key this mesh belongs to.
    pub terrain_mesh: Option<Mesh>, // Finished terrain mesh. None means this chunk only has instance children.
}

#[derive(Component)]
pub struct MapChunkMarker; // Marker attached to root entities spawned by the client map streamer.

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
        loading_chunks: HashMap::new(),
    });
}

pub(crate) fn update_client_map_streaming(state: Option<Res<State<InGameState>>>, player_query: Query<&Position, (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>)>, mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, runtime: Option<ResMut<MapStreamingRuntime>>,
) { // Load and unload map chunks based on player distance.
    let Some(state) = state else {
        return;
    };
    if !matches!(state.get(), InGameState::Loading | InGameState::Playing) {
        return;
    }
    let Some(mut runtime) = runtime else {
        return;
    };
    let Some(player_position) = player_query.iter().next() else {
        return;
    };
    let player_pos = player_position.0;

    let mut desired_chunks = HashSet::new();
    for (chunk_key, chunk_spec) in &runtime.catalog.chunks {
        if distance_point_to_chunk(player_pos, chunk_spec) <= STREAMING_LOAD_RADIUS {
            desired_chunks.insert(*chunk_key);
        }
    }

    let loaded_keys: Vec<MapChunkKey> = runtime.loaded_chunks.keys().copied().collect();
    for chunk_key in loaded_keys {
        let Some(chunk_spec) = runtime.catalog.chunks.get(&chunk_key) else {
            continue;
        };
        let distance = distance_point_to_chunk(player_pos, chunk_spec);
        if distance > STREAMING_UNLOAD_RADIUS && !desired_chunks.contains(&chunk_key) {
            if let Some(chunk_root) = runtime.loaded_chunks.remove(&chunk_key) {
                commands.entity(chunk_root).despawn();
            }
        }
    }

    let loading_keys: Vec<MapChunkKey> = runtime.loading_chunks.keys().copied().collect();
    for chunk_key in loading_keys {
        let Some(chunk_spec) = runtime.catalog.chunks.get(&chunk_key) else {
            continue;
        };
        let distance = distance_point_to_chunk(player_pos, chunk_spec);
        if distance > STREAMING_UNLOAD_RADIUS && !desired_chunks.contains(&chunk_key) {
            runtime.loading_chunks.remove(&chunk_key);
        }
    }

    let mut completed_chunks = Vec::new();
    let loading_keys: Vec<MapChunkKey> = runtime.loading_chunks.keys().copied().collect();
    for chunk_key in loading_keys {
        if completed_chunks.len() >= STREAMING_MAX_SPAWNS_PER_FRAME {
            break;
        }
        let Some(task) = runtime.loading_chunks.get_mut(&chunk_key) else {
            continue;
        };
        if let Some(result) = future::block_on(future::poll_once(task)) {
            completed_chunks.push(result);
        }
    }

    for result in completed_chunks {
        runtime.loading_chunks.remove(&result.chunk_key);
        if runtime.loaded_chunks.contains_key(&result.chunk_key) || !desired_chunks.contains(&result.chunk_key) {
            continue;
        }
        let Some(chunk_spec) = runtime.catalog.chunks.get(&result.chunk_key) else { continue; };
        let chunk_root = spawn_map_chunk(&mut commands, &asset_server, &mut meshes, &runtime.terrain_material, &map::MAP_COLLISION_CONFIG, result.chunk_key, chunk_spec, result.terrain_mesh, );
        runtime.loaded_chunks.insert(result.chunk_key, chunk_root);
    }

    let mut load_candidates: Vec<(MapChunkKey, f32)> = desired_chunks.iter()
        .filter(|chunk_key| { !runtime.loaded_chunks.contains_key(chunk_key) && !runtime.loading_chunks.contains_key(chunk_key) })
        .filter_map(|chunk_key| { runtime.catalog.chunks.get(chunk_key).map(|chunk_spec| (*chunk_key, distance_point_to_chunk(player_pos, chunk_spec))) })
        .collect();
    load_candidates.sort_unstable_by(|(_, a), (_, b)| a.total_cmp(b));

    let mut started = 0;
    for (chunk_key, _) in load_candidates {
        if started >= STREAMING_MAX_NEW_LOADS_PER_FRAME || runtime.loading_chunks.len() >= STREAMING_MAX_ASYNC_BUILDS {
            break;
        }
        let Some(chunk_spec) = runtime.catalog.chunks.get(&chunk_key) else {
            continue;
        };
        let terrain_config = map::MAP_COLLISION_CONFIG.terrain.as_ref();
        let terrain_chunk = chunk_spec.terrain.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            MapChunkBuildResult {
                chunk_key,
                terrain_mesh: terrain_config
                    .zip(terrain_chunk.as_ref())
                    .map(|(terrain, chunk)| build_terrain_chunk_mesh(terrain, chunk)),
            }
        });
        runtime.loading_chunks.insert(chunk_key, task);
        started += 1;
    }
}

pub(crate) fn cleanup_client_map_streaming(mut commands: Commands, runtime: Option<Res<MapStreamingRuntime>>) { // Remove all streamed chunk entities when leaving gameplay.
    let Some(runtime) = runtime else {
        return;
    };
    for chunk_root in runtime.loaded_chunks.values().copied() {
        commands.entity(chunk_root).despawn();
    }
    commands.remove_resource::<MapStreamingRuntime>();
    info!("[map] cleaned up client map streaming");
}

pub(crate) fn spawn_map_chunk(commands: &mut Commands, asset_server: &AssetServer, meshes: &mut Assets<Mesh>, terrain_material: &Handle<StandardMaterial>,
    config: &MapCollisionConfig, chunk_key: MapChunkKey, chunk_spec: &MapChunkSpec, terrain_mesh: Option<Mesh>, ) -> Entity {// Spawn one streamed terrain chunk and its attached instances.
    let chunk_root = commands
        .spawn((
            Name::new(format!("MapChunk_{}_{}", chunk_key.x, chunk_key.z)),
            MapChunkMarker,
            Transform::from_xyz(chunk_spec.origin.x, map::MAP_VISUAL_Y, chunk_spec.origin.z),
        )).id();
    commands.entity(chunk_root).with_children(|parent| {
        if let Some(mesh) = terrain_mesh {
            parent.spawn((
                Name::new(format!("MapChunkTerrain_{}_{}", chunk_key.x, chunk_key.z)),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(terrain_material.clone()),
                Transform::default(),
            ));
        }
        for &instance_index in &chunk_spec.glb_instance_indices {
            let instance = &config.glb_instances[instance_index];
            let local_translation = map::vec3_from_slice(&instance.translation) - chunk_spec.origin;
            parent.spawn((
                Name::new(format!(
                    "MapChunkInstance_{}_{}_{}",
                    chunk_key.x, chunk_key.z, instance.name
                )),
                SceneRoot(asset_server.load(
                    GltfAssetLabel::Scene(instance.scene).from_asset(instance.glb_path.clone()),
                )),
                Transform::from_translation(local_translation)
                    .with_rotation(Quat::from_rotation_y(instance.rotation_y))
                    .with_scale(map::vec3_from_slice(&instance.scale)),
            ));
            if let Some(collider) = &instance.collider {
                let collider_local_translation = local_translation
                    + Quat::from_rotation_y(instance.rotation_y)
                        * map::vec3_from_slice(&collider.translation);
                parent.spawn((
                    Name::new(format!(
                        "MapChunkInstanceCollider_{}_{}_{}",
                        chunk_key.x, chunk_key.z, instance.name
                    )),
                    map::MapColliderMarker,
                    RigidBody::Static,
                    Collider::cuboid(collider.size[0], collider.size[1], collider.size[2]),
                    CollisionLayers::new(WORLD_LAYER, PLAYER_LAYER),
                    Transform::from_translation(collider_local_translation).with_rotation(
                        Quat::from_rotation_y(instance.rotation_y + collider.rotation_y),
                    ),
                ));
            }
        }
    });
    chunk_root
}

fn build_terrain_chunk_mesh(terrain: &MapTerrainHeightfieldConfig, chunk: &MapTerrainChunkSpec) -> Mesh { // Build a chunk-local terrain mesh from the source heightmap.
    let cell_size_x = map::terrain_cell_size(terrain.rows, terrain.scale[0]);
    let cell_size_z = map::terrain_cell_size(terrain.columns, terrain.scale[2]);
    debug_assert!(
        (cell_size_x - cell_size_z).abs() < f32::EPSILON,
        "terrain visual expects uniform x/z cell size"
    );
    let mut heightmap = HeightMap::new(chunk.rows, chunk.columns, cell_size_x);
    for row in 0..chunk.rows {
        for column in 0..chunk.columns {
            heightmap.set(
                row,
                column,
                terrain.heights[chunk.rows_start + row][chunk.columns_start + column]
                    * terrain.scale[1],
            );
        }
    }
    HeightMapMeshBuilder::new().with_uv_tile_size(8.0).build(&heightmap)
}

pub(crate) fn build_map_streaming_catalog(config: &MapCollisionConfig) -> MapStreamingCatalog {// Precompute chunk boundaries and chunk ownership for all instances.
    let mut catalog = MapStreamingCatalog::default();
    let (chunk_size, terrain_origin) = if let Some(terrain) = &config.terrain {
        let total_cells_x = terrain.rows.saturating_sub(1);
        let total_cells_z = terrain.columns.saturating_sub(1);
        let chunk_cells_x = map::TERRAIN_CHUNK_CELL_COUNT.min(total_cells_x.max(1));
        let chunk_cells_z = map::TERRAIN_CHUNK_CELL_COUNT.min(total_cells_z.max(1));
        let cell_size_x = map::terrain_cell_size(terrain.rows, terrain.scale[0]);
        let cell_size_z = map::terrain_cell_size(terrain.columns, terrain.scale[2]);
        let chunk_size = Vec2::new(chunk_cells_x as f32 * cell_size_x, chunk_cells_z as f32 * cell_size_z, );
        let terrain_origin = Vec3::new(-map::terrain_axis_length(terrain.rows, terrain.scale[0]) * 0.5, 0.0, -map::terrain_axis_length(terrain.columns, terrain.scale[2]) * 0.5, );
        let chunk_count_x = total_cells_x.div_ceil(chunk_cells_x);
        let chunk_count_z = total_cells_z.div_ceil(chunk_cells_z);

        for chunk_x in 0..chunk_count_x {
            for chunk_z in 0..chunk_count_z {
                let rows_start = chunk_x * chunk_cells_x;
                let columns_start = chunk_z * chunk_cells_z;
                let rows = ((rows_start + chunk_cells_x).min(terrain.rows.saturating_sub(1)) - rows_start) + 1;
                let columns = ((columns_start + chunk_cells_z).min(terrain.columns.saturating_sub(1)) - columns_start) + 1;
                let origin = terrain_origin + Vec3::new(rows_start as f32 * cell_size_x, 0.0, columns_start as f32 * cell_size_z, );
                let half_size = Vec2::new((rows.saturating_sub(1)) as f32 * cell_size_x * 0.5, (columns.saturating_sub(1)) as f32 * cell_size_z * 0.5, );
                let center = origin + Vec3::new(half_size.x, 0.0, half_size.y);
                let key = MapChunkKey {
                    x: chunk_x as i32,
                    z: chunk_z as i32,
                };
                catalog.chunks.insert(
                    key,
                    MapChunkSpec {
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
                    },
                );
            }
        }
        (chunk_size, terrain_origin)
    } else {
        (Vec2::splat(map::STREAMING_FALLBACK_CHUNK_SIZE), Vec3::ZERO)
    };

    for (instance_index, instance) in config.glb_instances.iter().enumerate() {
        let chunk_key = chunk_key_for_position(
            map::vec3_from_slice(&instance.translation),
            terrain_origin,
            chunk_size,
        )
        .unwrap_or(MapChunkKey { x: 0, z: 0 });
        let entry = catalog.chunks.entry(chunk_key).or_insert_with(|| {
            let origin = terrain_origin
                + Vec3::new(
                    chunk_key.x as f32 * chunk_size.x,
                    0.0,
                    chunk_key.z as f32 * chunk_size.y,
                );
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

fn chunk_key_for_position(position: Vec3, origin: Vec3, chunk_size: Vec2) -> Option<MapChunkKey> {
    // Convert a world position into a chunk coordinate.
    if chunk_size.x <= 0.0 || chunk_size.y <= 0.0 {
        return None;
    }
    let local = position - origin;
    Some(MapChunkKey {
        x: (local.x / chunk_size.x).floor() as i32,
        z: (local.z / chunk_size.y).floor() as i32,
    })
}

pub(crate) fn distance_point_to_chunk(position: Vec3, chunk: &MapChunkSpec) -> f32 {
    // Measure planar distance from a point to the chunk bounds.
    let dx = (position.x - chunk.center.x).abs() - chunk.half_size.x;
    let dz = (position.z - chunk.center.z).abs() - chunk.half_size.y;
    let clamped_dx = dx.max(0.0);
    let clamped_dz = dz.max(0.0);
    (clamped_dx * clamped_dx + clamped_dz * clamped_dz).sqrt()
}
