// SettingsPlugin entry, msaa_from_samples, apply_window_settings, winit frame rate settings
use bevy::app::{App, Plugin, Startup, Update};
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin};
use bevy_inspector_egui::bevy_egui::EguiPrimaryContextPass;
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::Msaa;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use bevy::winit::{UpdateMode, WinitSettings};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use core::time::Duration;
use super::types::*;
use super::persist::load_or_create;
use super::ui::{toggle_settings_ui, draw_settings_ui};
use crate::vleue::feature::VleueSide;

pub struct SettingsPlugin { pub side: VleueSide }

impl Plugin for SettingsPlugin {
	fn build(&self, app: &mut App) {
		match self.side {
			VleueSide::Client => {
				let (settings, keybinds) = load_or_create();
				app.add_plugins((FrameTimeDiagnosticsPlugin::default(), SystemInformationDiagnosticsPlugin, RenderDiagnosticsPlugin));
				app.add_plugins(FpsOverlayPlugin { config: FpsOverlayConfig { enabled: settings.graphics.show_fps, text_config: TextFont { font_size: 16.0, ..default() }, text_color: Color::srgb(0.8, 0.92, 0.78), ..default() } });
				if settings.debug.show_inspector { app.add_plugins(WorldInspectorPlugin::new()); }
				app.insert_resource(winit_settings_from_game_settings(&settings));
				app.insert_resource(UiScale(settings.interface.ui_scale));
				app.insert_resource(settings);
				app.insert_resource(keybinds);
				app.init_resource::<SettingsUiState>();
				app.init_resource::<KeybindEditingState>();
				app.add_systems(Startup, (apply_window_settings, log_render_adapter_info));
				app.add_systems(Update, (toggle_settings_ui, sync_fps_overlay_config));
		    app.add_systems(EguiPrimaryContextPass, draw_settings_ui);
			}
			VleueSide::Server => {

            }
		}
	}
}


/// Log actual render backend and adapter selected by wgpu
fn log_render_adapter_info(adapter_info: Res<RenderAdapterInfo>) {
	info!("Render adapter: backend={:?}, name={}, device_type={:?}, driver={}, driver_info={}", adapter_info.backend, adapter_info.name, adapter_info.device_type, adapter_info.driver, adapter_info.driver_info);
}


/// Sync FPS Overlay toggle — Settings panel checkbox immediately affects Bevy UI FPS text
fn sync_fps_overlay_config(settings: Res<GameSettings>, mut overlay_config: ResMut<FpsOverlayConfig>) {
	if !settings.is_changed() || overlay_config.enabled == settings.graphics.show_fps { return; }
	overlay_config.enabled = settings.graphics.show_fps;
}

/// Calculate WinitSettings based on target frame rate — 0 means no limit, use game() default
fn winit_settings_from_game_settings(settings: &GameSettings) -> WinitSettings {
	if settings.graphics.target_fps == 0 { return WinitSettings::game(); }
	let frame_time = Duration::from_secs_f64(1.0 / settings.graphics.target_fps as f64);
	WinitSettings { focused_mode: UpdateMode::reactive(frame_time), unfocused_mode: UpdateMode::reactive_low_power(frame_time) }
}

/// Apply window settings — Set resolution, vertical sync mode and window mode
pub fn apply_window_settings(settings: Res<GameSettings>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
	let Ok(mut window) = windows.single_mut() else { return; };
	window.resolution.set(settings.window.width, settings.window.height);
	window.present_mode = if settings.graphics.vsync { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
	window.mode = match settings.window.window_mode { WindowModeSetting::Windowed => WindowMode::Windowed, WindowModeSetting::BorderlessFullscreen => WindowMode::BorderlessFullscreen(MonitorSelection::Primary) };
}



/// Convert MSAA sample count to Msaa enum — Only supports 1/2/4/8, other values default to Sample4
pub fn msaa_from_samples(samples: u32) -> Msaa {
	match samples { 1 => Msaa::Off, 2 => Msaa::Sample2, 4 => Msaa::Sample4, 8 => Msaa::Sample8, _ => Msaa::Sample4 }
}
