// Settings file I/O, load_or_create, write_settings_file, path helper functions
use super::types::*;
use std::fs;
use std::path::PathBuf;
use dirs::config_dir;

const GAME_SETTINGS_FILE_NAME: &str = "settings.toml";

/// Load or create config file — prefer reading existing config, create default if parse fails or doesn't exist
pub fn load_or_create() -> (GameSettings, PlayerKeybinds) {
	let path = settings_file_path(GAME_SETTINGS_FILE_NAME);
	if let Ok(content) = fs::read_to_string(&path) {
		if let Ok(file) = toml::from_str::<SettingsFile>(&content) {
			let settings = GameSettings {
				window: file.window, graphics: file.graphics, camera: file.camera,
				interface: file.interface, audio: file.audio, free_cam: file.free_cam, debug: file.debug,
			}.sanitized();
			let keybinds = PlayerKeybinds::from_file(file.keybinds);
			let _ = write_settings_file(&settings, &keybinds);
			return (settings, keybinds);
		}
	}
	let settings = GameSettings::default().sanitized();
	let keybinds = PlayerKeybinds::default();
	let _ = write_settings_file(&settings, &keybinds);
	(settings, keybinds)
}

/// Write settings to TOML file — create config directory and serialize save
pub fn write_settings_file(settings: &GameSettings, keybinds: &PlayerKeybinds) -> std::io::Result<()> {
	if let Some(config_dir) = settings_dir_path() {
		fs::create_dir_all(&config_dir)?;
	}
	let file = SettingsFile {
		window: settings.window.clone(), graphics: settings.graphics.clone(), camera: settings.camera.clone(),
		interface: settings.interface.clone(), audio: settings.audio.clone(),
		free_cam: settings.free_cam.clone(), debug: settings.debug.clone(),
		keybinds: keybinds.to_file(),
	};
	let content = toml::to_string_pretty(&file).unwrap_or_else(|_| String::new());
	fs::write(settings_file_path(GAME_SETTINGS_FILE_NAME), content)
}

/// Get user config directory path under the platform config directory.
pub fn settings_dir_path() -> Option<PathBuf> {
	config_dir().map(|path| path.join("bevy_lightyear_fps_example"))
}

/// Build full config file path — prefer user config directory, fallback to current directory if unavailable
fn settings_file_path(file_name: &str) -> PathBuf {
	settings_dir_path().map(|path| path.join(file_name)).unwrap_or_else(|| PathBuf::from(file_name))
}
