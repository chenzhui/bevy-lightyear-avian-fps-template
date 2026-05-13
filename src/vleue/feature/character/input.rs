use bevy::app::{FixedPreUpdate, Plugin, PreUpdate};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use lightyear::prelude::{Controlled, Predicted};
use crate::vleue::feature::character::movement::{CharacterAction, CharacterMarker};
use crate::vleue::feature::character::view::allow_fps_character_control;
use crate::vleue::feature::VleueSide;
use crate::vleue::feature::core::state::is_ingame_playing_and_connected;

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterAttackIntent { pub entity: Entity } // Melee attack intent, consumed by server combat system.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterShootIntent { pub entity: Entity } // Shoot intent, server settles damage, client plays local effects.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterInteractIntent { pub entity: Entity } // Single interaction intent, such as pickup or start extraction.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterInteractHeldIntent { pub entity: Entity } // Held interaction intent, such as holding interact key to rescue teammate.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterUseMedkitIntent { pub entity: Entity } // Use medkit intent, consumed by server channel action system.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterUseTeleportScrollIntent { pub entity: Entity } // Use teleport scroll intent, consumed by server channel action system.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterSkillPreviewIntent { pub entity: Entity } // Skill preview intent, client displays spell circle phantom.

#[derive(Message, Clone, Copy, Debug)]
pub struct CharacterSkillCastIntent { pub entity: Entity } // Skill confirm cast intent, server authoritative settles area damage.

pub struct CharacterInputPlugin { // Input intent plugin: centralized publishing of character discrete gameplay actions.
	pub side: VleueSide, // Input intent layer: translates bottom-level key states to gameplay intent.
}

impl Plugin for CharacterInputPlugin {
	fn build(&self, app: &mut App) {
		app.add_message::<CharacterAttackIntent>();
		app.add_message::<CharacterShootIntent>();
		app.add_message::<CharacterInteractIntent>();
		app.add_message::<CharacterInteractHeldIntent>();
		app.add_message::<CharacterUseMedkitIntent>();
		app.add_message::<CharacterUseTeleportScrollIntent>();
		app.add_message::<CharacterSkillPreviewIntent>();
		app.add_message::<CharacterSkillCastIntent>();
		match self.side {
			VleueSide::Client => {
				app.add_systems(PreUpdate, emit_local_character_input_intents.run_if(is_ingame_playing_and_connected).run_if(allow_fps_character_control));
			}
			VleueSide::Server => {
				app.add_systems(FixedPreUpdate, emit_server_character_input_intents);
			}
		}
	}
}

fn emit_local_character_input_intents(mut attack_writer: MessageWriter<CharacterAttackIntent>, mut shoot_writer: MessageWriter<CharacterShootIntent>, mut interact_writer: MessageWriter<CharacterInteractIntent>, mut interact_held_writer: MessageWriter<CharacterInteractHeldIntent>, mut medkit_writer: MessageWriter<CharacterUseMedkitIntent>, mut teleport_writer: MessageWriter<CharacterUseTeleportScrollIntent>, mut skill_preview_writer: MessageWriter<CharacterSkillPreviewIntent>, mut skill_cast_writer: MessageWriter<CharacterSkillCastIntent>, query: Query<(Entity, &ActionState<CharacterAction>), (With<CharacterMarker>, With<Controlled>, With<Predicted>)>) { // Client only publishes intent for local predicted player, avoids remote player triggering local effects repeatedly.
	emit_character_input_intents(&mut attack_writer, &mut shoot_writer, &mut interact_writer, &mut interact_held_writer, &mut medkit_writer, &mut teleport_writer, &mut skill_preview_writer, &mut skill_cast_writer, query.iter());
}

fn emit_server_character_input_intents(mut attack_writer: MessageWriter<CharacterAttackIntent>, mut shoot_writer: MessageWriter<CharacterShootIntent>, mut interact_writer: MessageWriter<CharacterInteractIntent>, mut interact_held_writer: MessageWriter<CharacterInteractHeldIntent>, mut medkit_writer: MessageWriter<CharacterUseMedkitIntent>, mut teleport_writer: MessageWriter<CharacterUseTeleportScrollIntent>, mut skill_preview_writer: MessageWriter<CharacterSkillPreviewIntent>, mut skill_cast_writer: MessageWriter<CharacterSkillCastIntent>, query: Query<(Entity, &ActionState<CharacterAction>), With<CharacterMarker>>) { // Server publishes intent for all authoritative characters, consumed by damage, pickup, extraction etc systems.
	emit_character_input_intents(&mut attack_writer, &mut shoot_writer, &mut interact_writer, &mut interact_held_writer, &mut medkit_writer, &mut teleport_writer, &mut skill_preview_writer, &mut skill_cast_writer, query.iter());
}

fn emit_character_input_intents<'a>(attack_writer: &mut MessageWriter<CharacterAttackIntent>, shoot_writer: &mut MessageWriter<CharacterShootIntent>, interact_writer: &mut MessageWriter<CharacterInteractIntent>, interact_held_writer: &mut MessageWriter<CharacterInteractHeldIntent>, medkit_writer: &mut MessageWriter<CharacterUseMedkitIntent>, teleport_writer: &mut MessageWriter<CharacterUseTeleportScrollIntent>, skill_preview_writer: &mut MessageWriter<CharacterSkillPreviewIntent>, skill_cast_writer: &mut MessageWriter<CharacterSkillCastIntent>, inputs: impl Iterator<Item = (Entity, &'a ActionState<CharacterAction>)>) { // Shared translation logic: converts bottom-level ActionState to stable gameplay intent messages.
	for (entity, action_state) in inputs {
		if action_state.just_pressed(&CharacterAction::Attack) {
			attack_writer.write(CharacterAttackIntent { entity });
		}
		if action_state.just_pressed(&CharacterAction::Shoot) {
			shoot_writer.write(CharacterShootIntent { entity });
		}
		if action_state.just_pressed(&CharacterAction::Interact) {
			interact_writer.write(CharacterInteractIntent { entity });
		}
		if action_state.pressed(&CharacterAction::Interact) {
			interact_held_writer.write(CharacterInteractHeldIntent { entity });
		}
		if action_state.just_pressed(&CharacterAction::UseMedkit) {
			medkit_writer.write(CharacterUseMedkitIntent { entity });
		}
		if action_state.just_pressed(&CharacterAction::UseTeleportScroll) {
			teleport_writer.write(CharacterUseTeleportScrollIntent { entity });
		}
		if action_state.pressed(&CharacterAction::SkillQ) {
			skill_preview_writer.write(CharacterSkillPreviewIntent { entity });
		}
		if action_state.just_released(&CharacterAction::SkillQ) {
			skill_cast_writer.write(CharacterSkillCastIntent { entity });
		}
	}
}
