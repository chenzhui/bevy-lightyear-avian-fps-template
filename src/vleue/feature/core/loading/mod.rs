use bevy::app::{App, Plugin, Update};
use bevy::asset::LoadState;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use lightyear::prelude::{Controlled, Predicted};
use crate::vleue::feature::character::movement::CharacterMarker;
use crate::vleue::feature::character::VleuePlayer;
use crate::vleue::feature::core::state::InGameState;
use crate::vleue::feature::VleueSide;

const MATCH_PRELOAD_ASSETS: &[(&str, &str)] = &[
    ("character_gltf", "girl.glb"),
];

pub struct LoadingPlugin {
    pub side: VleueSide, // Asset preload entry: unified handling of asset requests and completion detection after client InGame connection confirmation.
}

#[derive(Resource)]
struct MatchLoadingTracker {
    assets: Vec<TrackedAsset>, // Key resources that must complete before entering the current match.
}

struct TrackedAsset {
    key: &'static str, // Resource logical name, for log distinction.
    path: &'static str, // Resource path.
    handle: Handle<Gltf>, // glTF root resource handle, used to track scene and its recursive dependencies.
}

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        match self.side {
            VleueSide::Client => {
                app.add_systems(OnEnter(InGameState::Loading), begin_match_preload);
                app.add_systems(Update, update_match_preload.run_if(in_state(InGameState::Loading)));
                app.add_systems(OnExit(InGameState::Loading), cleanup_match_preload);
            }
            VleueSide::Server => {}
        }
    }
}

fn begin_match_preload(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut assets = Vec::with_capacity(MATCH_PRELOAD_ASSETS.len());
    for (key, path) in MATCH_PRELOAD_ASSETS {
        let handle: Handle<Gltf> = asset_server.load(*path);
        debug!("[loading] request preload key={} path={}", key, path);
        assets.push(TrackedAsset { key, path, handle });
    }
    commands.insert_resource(MatchLoadingTracker { assets });
    info!("[loading] entered InGameState::Loading, waiting for player + {} assets", MATCH_PRELOAD_ASSETS.len());
}

fn update_match_preload(mut next_state: ResMut<NextState<InGameState>>, asset_server: Res<AssetServer>, tracker: Option<Res<MatchLoadingTracker>>, player_query: Query<(), (With<VleuePlayer>, With<CharacterMarker>, With<Predicted>, With<Controlled>)>, ) {
    let Some(tracker) = tracker else { return; };
    if player_query.is_empty() {
        return;
    }
    for asset in &tracker.assets {
        if asset_server.is_loaded_with_dependencies(&asset.handle) {
            continue;
        }
        match asset_server.get_load_states(&asset.handle) {
            Some((load_state, dependency_state, recursive_dependency_state)) => {
                if let LoadState::Failed(err) = &load_state {
                    warn!("[loading] preload failed key={} path={} error={}", asset.key, asset.path, err);
                } else {
                    debug!(
                        "[loading] waiting key={} path={} load={:?} deps={:?} rec_deps={:?}",
                        asset.key,
                        asset.path,
                        load_state,
                        dependency_state,
                        recursive_dependency_state
                    );
                }
            }
            None => {
                debug!("[loading] waiting key={} path={} load_states=None", asset.key, asset.path);
            }
        }
        return;
    }
    info!("[loading] preload complete, entering InGameState::Playing");
    next_state.set(InGameState::Playing);
}

fn cleanup_match_preload(mut commands: Commands, tracker: Option<Res<MatchLoadingTracker>>) {
    if tracker.is_some() {
        commands.remove_resource::<MatchLoadingTracker>();
        debug!("[loading] leaving InGameState::Loading, cleared preload tracker");
    }
}
