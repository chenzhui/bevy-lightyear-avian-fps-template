use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};
use super::I18nResource;
use crate::vleue::feature::VleueSide;

pub struct I18nPlugin { pub side: VleueSide }

impl Plugin for I18nPlugin {
	fn build(&self, app: &mut App) {
		if self.side.is_client() {
			app.insert_resource(I18nResource::new("zh"));
			app.add_systems(Update, setup_fonts);
		}
	}
}

/// Load system font and register to egui — Prioritize Microsoft YaHei, fallback to SimHei/SimSun on failure
fn setup_fonts(mut contexts: EguiContexts, mut done: Local<bool>) {
	if *done { return; }
	let Ok(ctx) = contexts.ctx_mut() else { return; };

	let font_bytes = load_system_font();
	if font_bytes.is_empty() {
		bevy::log::warn!("Chinese font file not found, Chinese will display as boxes. Please install system font or manually place font to assets/fonts/");
		*done = true;
		return;
	}

	let mut fonts = egui::FontDefinitions::default();
	fonts.font_data.insert(
		"cn_font".to_owned(),
		std::sync::Arc::new(egui::FontData::from_owned(font_bytes)),
	);

	if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
		family.push("cn_font".to_owned());
	}
	if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
		family.push("cn_font".to_owned());
	}

	ctx.set_fonts(fonts);
	*done = true;
}

fn load_system_font() -> Vec<u8> {
	let paths = [
		"C:/Windows/Fonts/msyh.ttc",
		"C:/Windows/Fonts/simhei.ttf",
		"C:/Windows/Fonts/simsun.ttc",
	];
	for path in &paths {
		if let Ok(bytes) = std::fs::read(path) {
			return bytes;
		}
	}
	Vec::new()
}
