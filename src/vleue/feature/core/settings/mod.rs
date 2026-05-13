// Settings module entry — submodule declaration + public re-export (maintains external compatibility)
pub mod types;
pub(crate) mod persist; // File I/O, load_or_create, write_settings_file, path helper functions
pub(crate) mod ui;
pub mod plugin;

pub use types::{GameSettings, InputBinding, PlayerKeybinds, SettingsUiState, KeybindEditingState, KeybindField};
pub use plugin::{SettingsPlugin, msaa_from_samples, apply_window_settings};
