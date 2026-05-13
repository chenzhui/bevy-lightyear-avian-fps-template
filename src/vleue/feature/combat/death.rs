use bevy::prelude::*;
use bevy::ecs::message::MessageReader;
use serde::{Deserialize, Serialize};
use crate::vleue::feature::VleueSide;
use crate::vleue::feature::character::input::CharacterInteractHeldIntent;
use crate::vleue::feature::character::movement::CharacterMarker;
use avian3d::prelude::Position;

const DOWNED_DURATION: f32 = 30.0; // Total downed bleeding duration
const REVIVE_DURATION: f32 = 5.0;  // Required rescue duration
const REVIVE_DISTANCE: f32 = 2.0;  // Rescue distance

/// Character life state component (synced via Lightyear)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct LifeState {
    pub status: LifeStatus,
    pub downed_timer: f32,/// Bleeding timer after downed (seconds)
    pub revive_timer: f32, // Current rescue progress timer (seconds)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum LifeStatus {
    #[default]
    Alive,   // Normal combat
    Downed,  // Downed crawling (can be rescued)
    Dead,    // Completely dead
}

impl LifeState {
    pub fn alive() -> Self {
        Self {
            status: LifeStatus::Alive,
            downed_timer: 0.0,
            revive_timer: 0.0,
        }
    }
}

fn handle_downed_bleeding(time: Res<Time>, mut query: Query<&mut LifeState>) {
    for mut life in query.iter_mut().filter(|l| l.status == LifeStatus::Downed) {
        life.downed_timer += time.delta_secs();
        if life.downed_timer >= DOWNED_DURATION {
            life.status = LifeStatus::Dead;
        }
    }
}

fn handle_revival(
    time: Res<Time>,
    mut interact_held_events: MessageReader<CharacterInteractHeldIntent>,
    mut players: Query<(Entity, &mut LifeState, &Position), With<CharacterMarker>>,
) {
    let dt = time.delta_secs();
    let interact_held_entities: Vec<Entity> = interact_held_events.read().map(|event| event.entity).collect();
    let rescuers: Vec<Position> = players.iter()
        .filter(|(_, l, _)| l.status == LifeStatus::Alive)
        .filter(|(entity, _, _)| interact_held_entities.contains(entity))
        .map(|(_, _, pos)| *pos)
        .collect();

    for (_, mut life, pos) in players.iter_mut().filter(|(_, l, _)| l.status == LifeStatus::Downed) {
        let is_being_revived = rescuers.iter().any(|res_pos| pos.0.distance(res_pos.0) < REVIVE_DISTANCE);

        if is_being_revived {
            life.revive_timer += dt;
            if life.revive_timer >= REVIVE_DURATION {
                life.status = LifeStatus::Alive;
                life.downed_timer = 0.0;
                life.revive_timer = 0.0;
            }
        } else {
            life.revive_timer = life.revive_timer.max(0.0) - dt;
        }
    }
}

pub struct DeathPlugin {
    pub side: VleueSide,
}

use lightyear_replication::prelude::AppComponentExt;

impl Plugin for DeathPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<LifeState>();
        app.register_component::<LifeState>();
        
        if self.side == VleueSide::Server {
            app.add_systems(Update, (handle_downed_bleeding, handle_revival));
        }
    }
}
