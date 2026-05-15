// egui settings panel, keybind editing logic
use super::persist::write_settings_file;
use super::types::*;
use crate::vleue::feature::core::i18n::I18nResource;
use bevy::diagnostic::{
    DiagnosticsStore, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

#[derive(Default, PartialEq)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Audio,
    Game,
    Keybinds,
    Performance,
}

#[derive(Default)]
pub struct RenderDiagnosticsDebugLogState {
    frame_count: u32,
}

/// Toggle settings panel open/close
pub fn toggle_settings_ui(keyboard: Res<ButtonInput<KeyCode>>, mut ui_state: ResMut<SettingsUiState>, mut editing_state: ResMut<KeybindEditingState>) {
    if editing_state.editing_field.is_some() {
        if keyboard.just_pressed(KeyCode::Escape) {
            editing_state.editing_field = None;
        }
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        if !ui_state.opened {
            ui_state.opened = true;
            ui_state.page = SettingsUiPage::MainMenu;
        } else if ui_state.page == SettingsUiPage::Settings {
            ui_state.page = SettingsUiPage::MainMenu;
        } else {
            ui_state.opened = false;
        }
    }
}

/// Draw settings panel UI
pub fn draw_settings_ui(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<SettingsUiState>,
    mut settings: ResMut<GameSettings>,
    mut keybinds: ResMut<PlayerKeybinds>,
    mut editing_state: ResMut<KeybindEditingState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut i18n: ResMut<I18nResource>,
    diagnostics: Res<DiagnosticsStore>,
    mut active_tab: Local<SettingsTab>,
    mut render_diagnostics_log_state: Local<RenderDiagnosticsDebugLogState>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if !ui_state.opened {
        return;
    }
    let t = |key: &str| i18n.t(key);
    let keybind_conflicts = collect_keybind_conflicts(&keybinds, &i18n);
    let mut language_changed = false;

    if ui_state.page == SettingsUiPage::MainMenu {
        egui::Window::new(t("settings.title"))
            .default_size([260.0, 180.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
				if ui.add_sized([180.0, 44.0], egui::Button::new(t("settings.open_settings"))).clicked() {
                        ui_state.page = SettingsUiPage::Settings;
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized([180.0, 44.0], egui::Button::new(t("settings.quit_game")))
                        .clicked()
                    {
                        app_exit.write(AppExit::Success);
                    }
                    ui.add_space(12.0);
                    ui.label(t("settings.press_esc_close"));
                });
            });
        return;
    }

    egui::Window::new(t("settings.title")).default_size([400.0, 500.0]).collapsible(false).show(ctx, |ui| {
		// Top tab switching
		ui.horizontal(|ui| {
			ui.selectable_value(&mut *active_tab, SettingsTab::Graphics, t("settings.graphics"));
			ui.selectable_value(&mut *active_tab, SettingsTab::Audio, t("settings.audio"));
			ui.selectable_value(&mut *active_tab, SettingsTab::Game, t("settings.game"));
			ui.selectable_value(&mut *active_tab, SettingsTab::Keybinds, t("settings.keybinds"));
			ui.selectable_value(&mut *active_tab, SettingsTab::Performance, t("settings.performance"));
		});
		ui.separator();

		match *active_tab {
			SettingsTab::Graphics => {
				ui.checkbox(&mut settings.graphics.vsync, t("settings.vsync"));
				ui.add(egui::Slider::new(&mut settings.graphics.target_fps, 0..=360).text(t("settings.target_fps")));
				ui.add(egui::Slider::new(&mut settings.graphics.msaa_samples, 0..=8).text(t("settings.msaa_samples")));
				ui.checkbox(&mut settings.graphics.show_fps, t("settings.show_fps"));
			}
			SettingsTab::Audio => {
				ui.add(egui::Slider::new(&mut settings.audio.master_volume, 0.0..=1.0).text(t("settings.master_volume")));
				ui.add(egui::Slider::new(&mut settings.audio.music_volume, 0.0..=1.0).text(t("settings.music_volume")));
				ui.add(egui::Slider::new(&mut settings.audio.sfx_volume, 0.0..=1.0).text(t("settings.sfx_volume")));
			}
			SettingsTab::Game => {
				ui.horizontal(|ui| {
					ui.label(t("settings.language"));
					let previous_language = settings.interface.language.clone();
					egui::ComboBox::from_id_salt("settings_language")
						.selected_text(language_display_name(&settings.interface.language))
						.show_ui(ui, |ui| {
							ui.selectable_value(&mut settings.interface.language, "zh".to_string(), t("settings.language_zh"));
							ui.selectable_value(&mut settings.interface.language, "en".to_string(), t("settings.language_en"));
						});
					language_changed |= settings.interface.language != previous_language;
				});
				ui.add(egui::Slider::new(&mut settings.camera.mouse_sensitivity, 0.001..=0.1).text(t("settings.mouse_sensitivity")));
				ui.checkbox(&mut settings.camera.invert_y, t("settings.invert_y"));
				ui.add(egui::Slider::new(&mut settings.camera.first_person_fov_degrees, 40.0..=120.0).text(t("settings.fov")));
				ui.checkbox(&mut settings.interface.show_crosshair, t("settings.show_crosshair"));
				ui.checkbox(&mut settings.interface.show_health_hud, t("settings.show_health_hud"));
				ui.add(egui::Slider::new(&mut settings.interface.ui_scale, 0.5..=2.5).text(t("settings.ui_scale")));
			}
			SettingsTab::Keybinds => {
				ui.label(t("settings.keybinds_hint"));
				egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
					draw_keybind_row(ui, &*t("settings.move_up"), &mut keybinds.move_up, KeybindField::MoveUp, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.move_down"), &mut keybinds.move_down, KeybindField::MoveDown, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.move_left"), &mut keybinds.move_left, KeybindField::MoveLeft, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.move_right"), &mut keybinds.move_right, KeybindField::MoveRight, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.jump"), &mut keybinds.jump, KeybindField::Jump, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.interact"), &mut keybinds.interact, KeybindField::Interact, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.attack"), &mut keybinds.attack, KeybindField::Attack, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.shoot"), &mut keybinds.shoot, KeybindField::Shoot, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.skill_q"), &mut keybinds.skill_q, KeybindField::SkillQ, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.use_medkit"), &mut keybinds.use_medkit, KeybindField::UseMedkit, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.use_teleport_scroll"), &mut keybinds.use_teleport_scroll, KeybindField::UseTeleportScroll, &mut editing_state, &keyboard, &mouse);
					draw_keybind_row(ui, &*t("settings.toggle_inventory"), &mut keybinds.toggle_inventory, KeybindField::ToggleInventory, &mut editing_state, &keyboard, &mouse);
				});
				for conflict in &keybind_conflicts {
					ui.colored_label(egui::Color32::from_rgb(230, 80, 70), conflict);
				}
			}
			SettingsTab::Performance => {
				ui.label("Shows only this project and Bevy diagnostic data.");
				if ui.button(t("settings.copy_performance")).clicked() {
					ui.ctx().copy_text(build_performance_report(&diagnostics));
				}
				ui.separator();
				draw_diagnostic_row(ui, "FPS", diagnostic_smoothed(&diagnostics, &FrameTimeDiagnosticsPlugin::FPS).map(|value| format!("{value:.1}")).unwrap_or_else(|| "--".to_string()));
				draw_diagnostic_row(ui, "Frame time", diagnostic_smoothed(&diagnostics, &FrameTimeDiagnosticsPlugin::FRAME_TIME).map(|value| format!("{value:.2} ms")).unwrap_or_else(|| "--".to_string()));
				draw_diagnostic_row(ui, "Process CPU", diagnostic_smoothed(&diagnostics, &SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE).map(|value| format!("{value:.1} %")).unwrap_or_else(|| "--".to_string()));
				draw_diagnostic_row(ui, "Process RAM", diagnostic_smoothed(&diagnostics, &SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE).map(|value| format!("{value:.2} GiB")).unwrap_or_else(|| "--".to_string()));
				draw_diagnostic_row(ui, "System CPU", diagnostic_smoothed(&diagnostics, &SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE).map(|value| format!("{value:.1} %")).unwrap_or_else(|| "--".to_string()));
				draw_diagnostic_row(ui, "System RAM", diagnostic_smoothed(&diagnostics, &SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE).map(|value| format!("{value:.1} %")).unwrap_or_else(|| "--".to_string()));
				ui.separator();
				draw_render_diagnostics(ui, &diagnostics, &mut render_diagnostics_log_state);
				ui.separator();
				ui.label("GPU usage percentage is not shown here; Vulkan/DX12 render diagnostics show GPU elapsed time and pipeline statistics.");
			}
		}

		ui.separator();
		if ui.add_enabled(keybind_conflicts.is_empty(), egui::Button::new(t("settings.save_settings"))).clicked() {
			let sanitized = settings.clone().sanitized();
			let _ = write_settings_file(&sanitized, &keybinds);
			ui_state.opened = false;
			ui_state.page = SettingsUiPage::MainMenu;
		}
		ui.separator();
		if editing_state.editing_field.is_some() { ui.label(t("settings.press_any_key")); }
		else { ui.label(t("settings.press_esc_back")); }
	});
    if language_changed {
        let sanitized = settings.clone().sanitized();
        settings.interface.language = sanitized.interface.language;
        i18n.reload(&settings.interface.language);
    }
}

fn language_display_name(language: &str) -> &'static str {
    match language {
        "en" => "English",
        "zh" => "中文",
        _ => "中文",
    }
}

fn draw_diagnostic_row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label(value);
    });
}

fn diagnostic_smoothed(diagnostics: &DiagnosticsStore, path: &bevy::diagnostic::DiagnosticPath) -> Option<f64> {
    diagnostics
        .get(path)
        .and_then(|diagnostic| diagnostic.smoothed())
}

fn build_performance_report(diagnostics: &DiagnosticsStore) -> String {
    let rows = render_diagnostic_rows(diagnostics);
    let mut lines = vec![
        "Performance diagnostics".to_string(),
        format!(
            "FPS: {}",
            diagnostic_value(diagnostics, &FrameTimeDiagnosticsPlugin::FPS, 1, "")
        ),
        format!(
            "Frame time: {}",
            diagnostic_value(
                diagnostics,
                &FrameTimeDiagnosticsPlugin::FRAME_TIME,
                2,
                " ms"
            )
        ),
        format!(
            "Process CPU: {}",
            diagnostic_value(
                diagnostics,
                &SystemInformationDiagnosticsPlugin::PROCESS_CPU_USAGE,
                1,
                " %"
            )
        ),
        format!(
            "Process RAM: {}",
            diagnostic_value(
                diagnostics,
                &SystemInformationDiagnosticsPlugin::PROCESS_MEM_USAGE,
                2,
                " GiB"
            )
        ),
        format!(
            "System CPU: {}",
            diagnostic_value(
                diagnostics,
                &SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE,
                1,
                " %"
            )
        ),
        format!(
            "System RAM: {}",
            diagnostic_value(
                diagnostics,
                &SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE,
                1,
                " %"
            )
        ),
        format!(
            "Render diagnostics: {}",
            if rows.is_empty() {
                "--".to_string()
            } else {
                rows.len().to_string()
            }
        ),
        format!(
            "Max render GPU: {}",
            max_render_diagnostic(&rows, "/elapsed_gpu").unwrap_or_else(|| "--".to_string())
        ),
        format!(
            "Max render CPU: {}",
            max_render_diagnostic(&rows, "/elapsed_cpu").unwrap_or_else(|| "--".to_string())
        ),
        "Render rows:".to_string(),
    ];
    if rows.is_empty() {
        lines.push("--".to_string());
    } else {
        for (path, value, suffix) in rows {
            let value = value
                .map(|value| format!("{value:.2}{suffix}"))
                .unwrap_or_else(|| "--".to_string());
            lines.push(format!("{path}: {value}"));
        }
    }
    lines.push("Note: GPU usage percentage is not shown here; Vulkan/DX12 render diagnostics show GPU elapsed time and pipeline statistics.".to_string());
    lines.join("\n")
}

fn diagnostic_value(diagnostics: &DiagnosticsStore, path: &bevy::diagnostic::DiagnosticPath, precision: usize, suffix: &str) -> String {
    diagnostic_smoothed(diagnostics, path)
        .map(|value| format!("{value:.precision$}{suffix}"))
        .unwrap_or_else(|| "--".to_string())
}

fn render_diagnostic_rows(diagnostics: &DiagnosticsStore) -> Vec<(String, Option<f64>, String)> {
    let mut rows = Vec::new();
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().to_string();
        if path.starts_with("render/") {
            rows.push((path, diagnostic.smoothed(), diagnostic.suffix.to_string()));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

fn draw_render_diagnostics(ui: &mut egui::Ui, diagnostics: &DiagnosticsStore, log_state: &mut RenderDiagnosticsDebugLogState) {
    let mut all_paths = Vec::new();
    let rows = render_diagnostic_rows(diagnostics);
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().to_string();
        all_paths.push(path.clone());
    }
    log_state.frame_count = log_state.frame_count.wrapping_add(1);
    if log_state.frame_count == 1 || log_state.frame_count % 120 == 0 {
        let render_rows = rows.iter()
            .map(|(path, smoothed, suffix)| {
                let value = smoothed.map(|value| format!("{value:.3}{suffix}")).unwrap_or_else(|| "--".to_string());
                format!("{path}={value}")
            }).take(32).collect::<Vec<_>>();
        let sample_paths = all_paths.iter().map(String::as_str).take(32).collect::<Vec<_>>();
        // info!("Render diagnostics debug: all_count={}, render_count={}, sample_paths={:?}, render_rows={:?}", all_paths.len(), rows.len(), sample_paths, render_rows);
    }
    if rows.is_empty() {
        ui.label("Render diagnostics: --");
        return;
    }
    draw_diagnostic_row(ui, "Render diagnostics", rows.len().to_string());
	draw_diagnostic_row(ui, "Max render GPU", max_render_diagnostic(&rows, "/elapsed_gpu").unwrap_or_else(|| "--".to_string()));
	draw_diagnostic_row(ui, "Max render CPU", max_render_diagnostic(&rows, "/elapsed_cpu").unwrap_or_else(|| "--".to_string()));
    ui.separator();
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for (path, value, suffix) in rows {
                let value = value
                    .map(|value| format!("{value:.2}{suffix}"))
                    .unwrap_or_else(|| "--".to_string());
                draw_diagnostic_row(ui, &path, value);
            }
        });
}

fn max_render_diagnostic(rows: &[(String, Option<f64>, String)], suffix_filter: &str) -> Option<String> {
    rows.iter()
        .filter(|(path, value, _)| path.ends_with(suffix_filter) && value.is_some())
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal))
        .map(|(path, value, suffix)| format!("{} {:.3}{}", path, value.unwrap_or_default(), suffix))
}

/// Draw single keybind row — click button to enter edit mode, press any key to complete binding
fn draw_keybind_row(ui: &mut egui::Ui, label: &str, binding: &mut InputBinding, field: KeybindField, editing_state: &mut ResMut<KeybindEditingState>, keyboard: &Res<ButtonInput<KeyCode>>, mouse: &Res<ButtonInput<MouseButton>>, ) {
    let was_editing = editing_state.editing_field == Some(field);
    ui.horizontal(|ui| {
        ui.label(label);
        let is_editing = editing_state.editing_field == Some(field);
        let button_text = if is_editing {
            "Press key or mouse...".to_string()
        } else {
            input_binding_to_string(*binding)
        };
        let response = ui.button(&button_text);
        if response.clicked() && !is_editing {
            editing_state.editing_field = Some(field);
        }
    });
    if was_editing && editing_state.editing_field == Some(field) {
        if let Some(new_binding) = detect_any_input_press(keyboard, mouse) {
            *binding = new_binding;
            editing_state.editing_field = None;
        }
    }
}

fn collect_keybind_conflicts(keybinds: &PlayerKeybinds, i18n: &I18nResource) -> Vec<String> {
    let entries = [
        (i18n.t("settings.move_up"), keybinds.move_up),
        (i18n.t("settings.move_down"), keybinds.move_down),
        (i18n.t("settings.move_left"), keybinds.move_left),
        (i18n.t("settings.move_right"), keybinds.move_right),
        (i18n.t("settings.jump"), keybinds.jump),
        (i18n.t("settings.interact"), keybinds.interact),
        (i18n.t("settings.attack"), keybinds.attack),
        (i18n.t("settings.shoot"), keybinds.shoot),
        (i18n.t("settings.skill_q"), keybinds.skill_q),
		(i18n.t("settings.use_medkit"), keybinds.use_medkit), (i18n.t("settings.use_teleport_scroll"), keybinds.use_teleport_scroll),
		(i18n.t("settings.toggle_inventory"), keybinds.toggle_inventory),
    ];
    let mut conflicts = Vec::new();
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            if entries[i].1 == entries[j].1 {
				conflicts.push(i18n.t_args_multi("settings.keybind_conflict", &[("{first}", &entries[i].0), ("{second}", &entries[j].0), ("{binding}", &input_binding_to_string(entries[i].1))]));
            }
        }
    }
    conflicts
}

/// Detect any input press — iterate common keyboard and mouse buttons, return first just-pressed input
fn detect_any_input_press(keyboard: &Res<ButtonInput<KeyCode>>, mouse: &Res<ButtonInput<MouseButton>>) -> Option<InputBinding> {
    let candidates = [
        KeyCode::KeyA,
        KeyCode::KeyB,
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyE,
        KeyCode::KeyF,
        KeyCode::KeyG,
        KeyCode::KeyH,
        KeyCode::KeyI,
        KeyCode::KeyJ,
        KeyCode::KeyK,
        KeyCode::KeyL,
        KeyCode::KeyM,
        KeyCode::KeyN,
        KeyCode::KeyO,
        KeyCode::KeyP,
        KeyCode::KeyQ,
        KeyCode::KeyR,
        KeyCode::KeyS,
        KeyCode::KeyT,
        KeyCode::KeyU,
        KeyCode::KeyV,
        KeyCode::KeyW,
        KeyCode::KeyX,
        KeyCode::KeyY,
        KeyCode::KeyZ,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
        KeyCode::Digit0,
        KeyCode::Space,
        KeyCode::Tab,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ];
    for key in candidates {
        if keyboard.just_pressed(key) {
            return Some(InputBinding::Keyboard(key));
        }
    }
	let mouse_candidates = [MouseButton::Left, MouseButton::Right, MouseButton::Middle, MouseButton::Back, MouseButton::Forward];
    for button in mouse_candidates {
        if mouse.just_pressed(button) {
            return Some(InputBinding::Mouse(button));
        }
    }
    None
}
