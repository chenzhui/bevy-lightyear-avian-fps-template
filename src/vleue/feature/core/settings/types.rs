// Settings data model, enums, defaults, keybinds, serialization types, sanitized
use bevy::prelude::*;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use serde::{Deserialize, Serialize};

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GameSettings {
	pub window: WindowSettings, // Window related settings.
	pub graphics: GraphicsSettings, // Graphics and frame rate related settings.
	pub camera: CameraSettings, // First-person view related settings.
	pub interface: InterfaceSettings, // HUD and crosshair related settings.
	pub audio: AudioSettings, // Volume related settings, can be extended with real audio control later.
	pub free_cam: FreeCamSettings, // Free camera settings.
	pub debug: DebugSettings, // Development debug related settings.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowSettings {
	pub width: f32, // Client window width.
	pub height: f32, // Client window height.
	pub window_mode: WindowModeSetting, // Window mode.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphicsSettings {
	pub vsync: bool, // Whether to enable vertical sync.
	pub target_fps: u32, // Target frame rate, 0 means no active limit.
	pub msaa_samples: u32, // MSAA sample count, only supports 1/2/4/8.
	pub show_fps: bool, // Whether to show FPS text.
	pub quality: GraphicsQuality, // Graphics quality tier, serves as future rendering settings entry point.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CameraSettings {
	pub mouse_sensitivity: f32, // First-person mouse sensitivity.
	pub invert_y: bool, // Whether to invert first-person vertical view.
	pub first_person_fov_degrees: f32, // First-person camera field of view angle.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceSettings {
	pub show_crosshair: bool, // Whether to show crosshair.
	pub show_health_hud: bool, // Whether to show top-left health bar HUD.
	pub ui_scale: f32, // UI global scale.
	#[serde(default = "default_language")]
	pub language: String, // UI language code.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioSettings {
	pub master_volume: f32, // Master volume.
	pub music_volume: f32, // Music volume.
	pub sfx_volume: f32, // Sound effects volume.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FreeCamSettings {
	pub move_speed: f32, // Free camera movement speed.
	pub look_sensitivity: f32, // Free camera mouse sensitivity.
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugSettings {
	pub show_inspector: bool, // Whether to load World Inspector.
	pub show_physics_gizmos: bool, // Reserved: whether to show physics debug lines.
	pub show_network_stats: bool, // Reserved: whether to show network status.
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WindowModeSetting {
	Windowed,
	BorderlessFullscreen,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphicsQuality {
	Low,
	Medium,
	High,
	Ultra,
}

#[derive(Resource, Clone, Debug)]
pub struct PlayerKeybinds {
	pub move_up: InputBinding, // Move forward input.
	pub move_down: InputBinding, // Move backward input.
	pub move_left: InputBinding, // Move left input.
	pub move_right: InputBinding, // Move right input.
	pub jump: InputBinding, // Jump input.
	pub interact: InputBinding, // Interact input.
	pub attack: InputBinding, // Melee attack input.
	pub shoot: InputBinding, // Shoot input.
	pub skill_q: InputBinding, // Q skill input.
	pub use_medkit: InputBinding, // Use medkit input.
	pub use_teleport_scroll: InputBinding, // Use teleport scroll input.
	pub toggle_inventory: InputBinding, // Open or close inventory and equipment interface.
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputBinding {
	Keyboard(KeyCode), // Keyboard key binding.
	Mouse(MouseButton), // Mouse button binding.
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SettingsFile {
	#[serde(default)]pub(crate) window: WindowSettings,
	#[serde(default)]pub(crate) graphics: GraphicsSettings,
	#[serde(default)]pub(crate) camera: CameraSettings,
	#[serde(default)]pub(crate) interface: InterfaceSettings,
	#[serde(default)]pub(crate) audio: AudioSettings,
	#[serde(default)]pub(crate) free_cam: FreeCamSettings,
	#[serde(default)]pub(crate) debug: DebugSettings,
	#[serde(default)]pub(crate) keybinds: PlayerKeybindsFile,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct PlayerKeybindsFile {
	pub(crate) move_up: String,
	pub(crate) move_down: String,
	pub(crate) move_left: String,
	pub(crate) move_right: String,
	pub(crate) jump: String,
	pub(crate) interact: String,
	pub(crate) attack: String,
	pub(crate) shoot: String,
	#[serde(default = "default_skill_q_key")]
	pub(crate) skill_q: String,
	pub(crate) use_medkit: String,
	pub(crate) use_teleport_scroll: String,
	#[serde(default = "default_toggle_inventory_key")]
	pub(crate) toggle_inventory: String,
}

impl Default for GameSettings {
	fn default() -> Self {
		Self {
			window: WindowSettings::default(),
			graphics: GraphicsSettings::default(),
			camera: CameraSettings::default(),
			interface: InterfaceSettings::default(),
			audio: AudioSettings::default(),
			free_cam: FreeCamSettings::default(),
			debug: DebugSettings::default(),
		}
	}
}

impl Default for WindowSettings {
	fn default() -> Self {
		Self { width: 1600.0, height: 900.0, window_mode: WindowModeSetting::Windowed }
	}
}

impl Default for CameraSettings {
	fn default() -> Self {
		Self { mouse_sensitivity: 0.01, invert_y: false, first_person_fov_degrees: 75.0 }
	}
}

impl Default for GraphicsSettings {
	fn default() -> Self {
		Self { vsync: true, target_fps: 0, msaa_samples: 4, show_fps: false, quality: GraphicsQuality::Medium }
	}
}

impl Default for InterfaceSettings {
	fn default() -> Self {
		Self { show_crosshair: true, show_health_hud: true, ui_scale: 1.0, language: default_language() }
	}
}

impl Default for AudioSettings {
	fn default() -> Self {
		Self { master_volume: 1.0, music_volume: 0.8, sfx_volume: 0.9 }
	}
}

impl Default for FreeCamSettings {
	fn default() -> Self {
		Self { move_speed: 20.0, look_sensitivity: 0.003 }
	}
}

impl Default for DebugSettings {
	fn default() -> Self {
		Self { show_inspector: false, show_physics_gizmos: false, show_network_stats: false }
	}
}

impl Default for WindowModeSetting {
	fn default() -> Self { Self::Windowed }
}

impl Default for GraphicsQuality {
	fn default() -> Self { Self::Medium }
}

impl Default for PlayerKeybinds {
	fn default() -> Self {
		Self {
			move_up: KeyCode::KeyW.into(), move_down: KeyCode::KeyS.into(),
			move_left: KeyCode::KeyA.into(), move_right: KeyCode::KeyD.into(),
			jump: KeyCode::Space.into(), interact: KeyCode::KeyF.into(),
			attack: KeyCode::KeyJ.into(), shoot: MouseButton::Left.into(),
			skill_q: KeyCode::KeyQ.into(),
			use_medkit: KeyCode::Digit1.into(), use_teleport_scroll: KeyCode::Digit2.into(),
			toggle_inventory: KeyCode::Tab.into(),
		}
	}
}

impl Default for PlayerKeybindsFile {
	fn default() -> Self { PlayerKeybinds::default().to_file() }
}

impl Default for SettingsFile {
	fn default() -> Self {
		let s = GameSettings::default();
		Self { window: s.window, graphics: s.graphics, camera: s.camera, interface: s.interface, audio: s.audio, free_cam: s.free_cam, debug: s.debug, keybinds: PlayerKeybindsFile::default() }
	}
}

impl GameSettings {
	/// Validate and correct config values — ensure all values are within valid ranges
	pub fn sanitized(mut self) -> Self {
		self.window.width = self.window.width.max(640.0);
		self.window.height = self.window.height.max(480.0);
		self.graphics.msaa_samples = match self.graphics.msaa_samples {
			1 | 2 | 4 | 8 => self.graphics.msaa_samples,
			0 => 1,
			value if value <= 2 => 2,
			value if value <= 4 => 4,
			_ => 8,
		};
		self.graphics.target_fps = self.graphics.target_fps.min(360);
		self.interface.ui_scale = self.interface.ui_scale.clamp(0.5, 2.5);
		self.camera.mouse_sensitivity = self.camera.mouse_sensitivity.clamp(0.0005, 0.2);
		self.camera.first_person_fov_degrees = self.camera.first_person_fov_degrees.clamp(40.0, 120.0);
		self.audio.master_volume = self.audio.master_volume.clamp(0.0, 1.0);
		self.audio.music_volume = self.audio.music_volume.clamp(0.0, 1.0);
		self.audio.sfx_volume = self.audio.sfx_volume.clamp(0.0, 1.0);
		self.free_cam.move_speed = self.free_cam.move_speed.clamp(1.0, 200.0);
		self.free_cam.look_sensitivity = self.free_cam.look_sensitivity.clamp(0.0005, 0.1);
		if !matches!(self.interface.language.as_str(), "zh" | "en") {
			self.interface.language = default_language();
		}
		self
	}
}

impl PlayerKeybinds {
	pub(crate) fn to_file(&self) -> PlayerKeybindsFile {
		PlayerKeybindsFile {
			move_up: input_binding_to_string(self.move_up), move_down: input_binding_to_string(self.move_down),
			move_left: input_binding_to_string(self.move_left), move_right: input_binding_to_string(self.move_right),
			jump: input_binding_to_string(self.jump), interact: input_binding_to_string(self.interact),
			attack: input_binding_to_string(self.attack), shoot: input_binding_to_string(self.shoot),
			skill_q: input_binding_to_string(self.skill_q),
			use_medkit: input_binding_to_string(self.use_medkit), use_teleport_scroll: input_binding_to_string(self.use_teleport_scroll),
			toggle_inventory: input_binding_to_string(self.toggle_inventory),
		}
	}

	pub(crate) fn from_file(file: PlayerKeybindsFile) -> Self {
		let defaults = Self::default();
		Self {
			move_up: parse_input_binding(&file.move_up).unwrap_or(defaults.move_up),
			move_down: parse_input_binding(&file.move_down).unwrap_or(defaults.move_down),
			move_left: parse_input_binding(&file.move_left).unwrap_or(defaults.move_left),
			move_right: parse_input_binding(&file.move_right).unwrap_or(defaults.move_right),
			jump: parse_input_binding(&file.jump).unwrap_or(defaults.jump),
			interact: parse_input_binding(&file.interact).unwrap_or(defaults.interact),
			attack: parse_input_binding(&file.attack).unwrap_or(defaults.attack),
			shoot: parse_input_binding(&file.shoot).unwrap_or(defaults.shoot),
			skill_q: parse_input_binding(&file.skill_q).unwrap_or(defaults.skill_q),
			use_medkit: parse_input_binding(&file.use_medkit).unwrap_or(defaults.use_medkit),
			use_teleport_scroll: parse_input_binding(&file.use_teleport_scroll).unwrap_or(defaults.use_teleport_scroll),
			toggle_inventory: parse_input_binding(&file.toggle_inventory).unwrap_or(defaults.toggle_inventory),
		}
	}
}

impl From<KeyCode> for InputBinding {
	fn from(value: KeyCode) -> Self { Self::Keyboard(value) }
}

impl From<MouseButton> for InputBinding {
	fn from(value: MouseButton) -> Self { Self::Mouse(value) }
}

impl InputBinding {
	pub fn just_pressed(self, keyboard: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
		match self {
			Self::Keyboard(key) => keyboard.just_pressed(key),
			Self::Mouse(button) => mouse.just_pressed(button),
		}
	}
}

/// Settings panel UI state resource
#[derive(Resource, Default)]
pub struct SettingsUiState {
	pub opened: bool, // Whether settings panel is open.
	pub page: SettingsUiPage, // Currently showing main menu or settings page.
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsUiPage {
	#[default]
	MainMenu,
	Settings,
}

/// Keybind editing state resource
#[derive(Resource, Default)]
pub struct KeybindEditingState {
	pub editing_field: Option<KeybindField>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum KeybindField {
	MoveUp, MoveDown, MoveLeft, MoveRight,
	Jump, Interact, Attack, Shoot,
	SkillQ, UseMedkit, UseTeleportScroll, ToggleInventory,
}

fn default_skill_q_key() -> String { "Q".to_string() }

fn default_toggle_inventory_key() -> String { "Tab".to_string() }

fn default_language() -> String { "zh".to_string() }

pub(crate) fn parse_input_binding(value: &str) -> Option<InputBinding> {
	parse_mouse_button(value).map(InputBinding::Mouse).or_else(|| parse_keycode(value).map(InputBinding::Keyboard))
}

pub(crate) fn parse_mouse_button(value: &str) -> Option<MouseButton> {
	match value.trim().to_ascii_uppercase().as_str() {
		"MOUSELEFT" | "MOUSE_LEFT" | "LEFTMOUSE" | "LEFT_MOUSE" | "LMB" => Some(MouseButton::Left),
		"MOUSERIGHT" | "MOUSE_RIGHT" | "RIGHTMOUSE" | "RIGHT_MOUSE" | "RMB" => Some(MouseButton::Right),
		"MOUSEMIDDLE" | "MOUSE_MIDDLE" | "MIDDLEMOUSE" | "MIDDLE_MOUSE" | "MMB" => Some(MouseButton::Middle),
		"MOUSEBACK" | "MOUSE_BACK" => Some(MouseButton::Back),
		"MOUSEFORWARD" | "MOUSE_FORWARD" => Some(MouseButton::Forward),
		_ => None,
	}
}

pub(crate) fn parse_keycode(value: &str) -> Option<KeyCode> {
	match value.trim().to_ascii_uppercase().as_str() {
		"W" | "KEYW" => Some(KeyCode::KeyW), "A" | "KEYA" => Some(KeyCode::KeyA),
		"S" | "KEYS" => Some(KeyCode::KeyS), "D" | "KEYD" => Some(KeyCode::KeyD),
		"F" | "KEYF" => Some(KeyCode::KeyF), "J" | "KEYJ" => Some(KeyCode::KeyJ),
		"K" | "KEYK" => Some(KeyCode::KeyK), "P" | "KEYP" => Some(KeyCode::KeyP),
		"L" | "KEYL" => Some(KeyCode::KeyL), "Q" | "KEYQ" => Some(KeyCode::KeyQ),
		"E" | "KEYE" => Some(KeyCode::KeyE), "R" | "KEYR" => Some(KeyCode::KeyR),
		"M" | "KEYM" => Some(KeyCode::KeyM), "N" | "KEYN" => Some(KeyCode::KeyN),
		"SPACE" => Some(KeyCode::Space),
		"1" | "DIGIT1" => Some(KeyCode::Digit1), "2" | "DIGIT2" => Some(KeyCode::Digit2),
		"3" | "DIGIT3" => Some(KeyCode::Digit3), "4" | "DIGIT4" => Some(KeyCode::Digit4),
		"5" | "DIGIT5" => Some(KeyCode::Digit5), "6" | "DIGIT6" => Some(KeyCode::Digit6),
		"7" | "DIGIT7" => Some(KeyCode::Digit7), "8" | "DIGIT8" => Some(KeyCode::Digit8),
		"9" | "DIGIT9" => Some(KeyCode::Digit9), "0" | "DIGIT0" => Some(KeyCode::Digit0),
		"TAB" => Some(KeyCode::Tab), "ESC" | "ESCAPE" => Some(KeyCode::Escape),
		"LEFT" | "ARROWLEFT" => Some(KeyCode::ArrowLeft), "RIGHT" | "ARROWRIGHT" => Some(KeyCode::ArrowRight),
		"UP" | "ARROWUP" => Some(KeyCode::ArrowUp), "DOWN" | "ARROWDOWN" => Some(KeyCode::ArrowDown),
		_ => None,
	}
}

pub(crate) fn input_binding_to_string(binding: InputBinding) -> String {
	match binding {
		InputBinding::Keyboard(key) => keycode_to_string(key),
		InputBinding::Mouse(button) => mouse_button_to_string(button),
	}
}

pub(crate) fn mouse_button_to_string(button: MouseButton) -> String {
	match button {
		MouseButton::Left => "MouseLeft",
		MouseButton::Right => "MouseRight",
		MouseButton::Middle => "MouseMiddle",
		MouseButton::Back => "MouseBack",
		MouseButton::Forward => "MouseForward",
		MouseButton::Other(value) => return format!("MouseOther{}", value),
	}.to_string()
}

pub(crate) fn keycode_to_string(key: KeyCode) -> String {
	match key {
		KeyCode::KeyW => "W",
		KeyCode::KeyA => "A",
		KeyCode::KeyS => "S",
		KeyCode::KeyD => "D",
		KeyCode::KeyF => "F",
		KeyCode::KeyJ => "J",
		KeyCode::KeyK => "K",
		KeyCode::KeyP => "P",
		KeyCode::KeyL => "L",
		KeyCode::KeyQ => "Q",
		KeyCode::KeyE => "E",
		KeyCode::KeyR => "R",
		KeyCode::KeyM => "M",
		KeyCode::KeyN => "N",
		KeyCode::Space => "Space",
		KeyCode::Digit1 => "1",
		KeyCode::Digit2 => "2",
		KeyCode::Digit3 => "3",
		KeyCode::Digit4 => "4",
		KeyCode::Digit5 => "5",
		KeyCode::Digit6 => "6",
		KeyCode::Digit7 => "7",
		KeyCode::Digit8 => "8",
		KeyCode::Digit9 => "9",
		KeyCode::Digit0 => "0",
		KeyCode::Tab => "Tab",
		KeyCode::Escape => "Escape",
		KeyCode::ArrowLeft => "Left",
		KeyCode::ArrowRight => "Right",
		KeyCode::ArrowUp => "Up",
		KeyCode::ArrowDown => "Down",
		_ => return format!("{:?}", key),
	}.to_string()
}
