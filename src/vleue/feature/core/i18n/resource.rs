use bevy::prelude::*;
use std::collections::HashMap;

/// I18n resource — Flattens TOML into HashMap at startup, only O(1) lookup at runtime
#[derive(Resource)]
pub struct I18nResource {
	pub lang: String,
	dictionary: HashMap<String, String>,
}

impl I18nResource {
	pub fn new(lang_code: &str) -> Self {
		let path = format!("assets/i18/{}.toml", lang_code);
		let content = std::fs::read_to_string(&path).unwrap_or_default();
		let toml_value: toml::Value = toml::from_str(&content).unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
		let dictionary = flatten_toml("", &toml_value);
		Self { lang: lang_code.to_string(), dictionary }
	}

	pub fn reload(&mut self, lang_code: &str) {
		*self = Self::new(lang_code);
	}

	/// Basic translation — Fallback to display key itself when not found, preventing blank UI
	pub fn t(&self, key: &str) -> String {
		self.dictionary.get(key).cloned().unwrap_or_else(|| key.to_string())
	}

	/// Translation with single parameter replacement — e.g. t_args("lobby.queuing", queue_size)
	pub fn t_args(&self, key: &str, arg: &str) -> String {
		self.t(key).replace("{}", arg)
	}

	/// Translation with multiple parameter replacement
	pub fn t_args_multi(&self, key: &str, args: &[(&str, &str)]) -> String {
		let mut text = self.t(key);
		for (k, v) in args {
			text = text.replace(k, v);
		}
		text
	}
}

/// Recursively flatten TOML — Flattens hierarchical structure into "section.key" form
fn flatten_toml(prefix: &str, value: &toml::Value) -> HashMap<String, String> {
	let mut map = HashMap::new();
	match value {
		toml::Value::Table(table) => {
			for (k, v) in table {
				let new_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
				map.extend(flatten_toml(&new_prefix, v));
			}
		}
		toml::Value::String(s) => { map.insert(prefix.to_string(), s.clone()); }
		toml::Value::Integer(i) => { map.insert(prefix.to_string(), i.to_string()); }
		toml::Value::Float(f) => { map.insert(prefix.to_string(), f.to_string()); }
		toml::Value::Boolean(b) => { map.insert(prefix.to_string(), b.to_string()); }
		_ => {}
	}
	map
}
