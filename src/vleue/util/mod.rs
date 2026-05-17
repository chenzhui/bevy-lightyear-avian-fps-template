// use bevy::app::{App, Plugin, Update};
// use bevy::prelude::{Entity, KeyCode, Query, ResMut, Resource, World};
// use crate::vleue::feature::core::{connection, health, i18n, net, room, settings, state};
// use crate::vleue::feature::VleueSide;
//
pub mod log_util;
pub mod env;
//
// #[derive(Resource)]
// struct TargetEntity {
// 	pub entity: Entity,  // ✅ 加 pub
// }
//
// pub struct UtilPlugin {
// 	pub side: VleueSide, // core trunk: unified network, room, state and other underlying capabilities.
// }
//
// impl Plugin for UtilPlugin{
// 	fn build(&self, app: &mut App) {
// 		// app.insert_resource(TargetEntity);
// 		app.add_systems(Update,inspect_entity);
//
// 	}
// }
//
//
// fn inspect_entity(world: &mut World) {
// 	let target = world.resource::<TargetEntity>();
//
// 	if let Ok(entity_ref) = world.get_entity(target) {
// 		let archetype = entity_ref.archetype();
// 		for component_id in archetype.components() {
// 			if let Some(info) = world.components().get_info(component_id) {
// 				println!("{}", info.name());
// 			}
// 		}
// 	}
// }


// #[derive(Resource, Default)]
// pub struct InspectQueue(Vec<Entity>);
//
// fn queue_inspect(mut queue: ResMut<InspectQueue>, query: Query<Entity>, ) {
// 	if let Some(entity) = query.iter().next() {
// 		queue.0.push(entity);
// 		println!("已加入检查队列: {:?}", entity);
// 	}
// }
//
//
// fn process_inspect(world: &mut World) {
// 	let targets: Vec<Entity> = { let queue = world.resource::<InspectQueue>();
// 		queue.0.clone()
// 	};
//
// 	for target in targets {
// 		if let Ok(entity_ref) = world.get_entity(target) {
// 			println!("=== Entity {:?} ===", target);
// 			for component_id in entity_ref.archetype().components() {
// 				if let Some(info) = world.components().get_info(*component_id) {
// 					println!("  组件: {:?}", info.name());
// 				}
// 			}
// 		}
// 	}
//
// 	// 清空队列
// 	world.resource_mut::<InspectQueue>().0.clear();
// }


// pub struct UtilPlugin {}
//
// impl Plugin for UtilPlugin{
// 	fn build(&self, app: &mut App) {
// 		app.add_plugins(
// 			app.add_systems(Update, inspect_entity_system());
// 		)
// 	}
// }


// pub fn inspect_entity(world: &World, entity: Entity) {
// 	if let Ok(entity_ref) = world.get_entity(entity) {
// 		// 遍历所有组件
// 		for component_id in entity_ref.archetype().components() {
// 			// 获取组件信息
// 			if let Some(component_info) = world.components().get_info(*component_id) {
// 				println!("Component: {:?}", component_info.name());
// 			}
// 		}
// 	} else {
// 		println!("Entity {:?} does not exist", entity);
// 	}
// }