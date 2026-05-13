use bevy::prelude::*;
use bevy::camera_controller::free_camera::{FreeCamera, FreeCameraPlugin, FreeCameraState};
use avian3d::prelude::*;
use bevy::gizmos::config::GizmoConfig;
use lightyear_replication::prelude::{Room, RoomEvent, RoomTarget};
use crate::vleue::feature::core::connection::GameRoomId;

pub struct ServerDebugPlugin {
    pub room_id: Option<u64>,
}

impl Plugin for ServerDebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FreeCameraPlugin);
        app.add_plugins(PhysicsDebugPlugin);
        app.insert_gizmo_config(
            PhysicsGizmos {
                aabb_color: None,
                collider_color: Some(Color::WHITE),
                contact_point_color: None,
                contact_normal_color: None,
                joint_anchor_color: None,
                joint_separation_color: None,
                raycast_color: None,
                raycast_point_color: None,
                raycast_normal_color: None,
                shapecast_color: None,
                shapecast_shape_color: None,
                shapecast_point_color: None,
                shapecast_normal_color: None,
                island_color: None,
                ..default()
            },
            GizmoConfig::default(),
        );
        if self.room_id.is_some() {
            app.add_systems(Update, add_server_camera_to_room);
        }
        app.insert_resource(ServerDebugRoomId(self.room_id.unwrap_or(0)));
        app.add_systems(Startup, spawn_debug_camera);
        app.add_systems(Startup, spawn_ui);
    }
}

#[derive(Component)]
struct ServerDebugCamera;

#[derive(Component)]
struct ServerDebugCameraRoomAttached;

#[derive(Resource, Debug, Clone, Copy)]
struct ServerDebugRoomId(pub u64);

#[derive(Component)]
struct UiTextMarker;

fn spawn_debug_camera(mut commands: Commands, debug_room: Res<ServerDebugRoomId>) {
    commands.spawn((
        Name::new(format!("ServerDebugCamera_Room{}", debug_room.0)),
        ServerDebugCamera,
        GameRoomId(debug_room.0),
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        FreeCamera {
            sensitivity: 0.2,
            friction: 25.0,
            walk_speed: 10.0,
            run_speed: 30.0,
            ..default()
        },
    ));
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        UiTextMarker,
        Text::new("Server Debug Mode\nWASD/QE: Move\nMouse: Look\nB: Toggle Camera"),
        TextColor(Color::srgb(0.0, 1.0, 0.0)),
    ));
}

fn add_server_camera_to_room(mut commands: Commands, cameras: Query<(Entity, &GameRoomId), (With<ServerDebugCamera>, Without<ServerDebugCameraRoomAttached>)>, rooms: Query<(Entity, &GameRoomId), With<Room>>) {
    for (camera_entity, camera_room) in &cameras {
        let Some((room_entity, _)) = rooms.iter().find(|(_, room_id)| room_id.0 == camera_room.0) else { continue; };
        commands.trigger(RoomEvent {
            target: RoomTarget::AddEntity(camera_entity),
            room: room_entity,
        });
        commands.entity(camera_entity).insert(ServerDebugCameraRoomAttached);
        info!("[server] debug camera joined room {}", camera_room.0);
    }
}
