use std::collections::HashMap;
use std::path::Path;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub use self::entry::{Entry, Part as EntryPart, PloverCommand, SpecialPunct};
pub use self::strokes::Strokes;
use crate::keys::Keys;

mod entry;
mod strokes;

#[derive(Debug)]
pub struct Dict {
	map: HashMap<Strokes, Entry>,
	max_strokes: usize,
}

impl<'de> Deserialize<'de> for Dict {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		struct MapVisitor {}

		impl<'de> Visitor<'de> for MapVisitor {
			type Value = Dict;

			fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				formatter.write_str("a string-to-string map")
			}

			fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
				let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));

				let mut max_strokes = 1;

				while let Some((key, value)) = access.next_entry::<Strokes, Entry>()? {
					if let Some(old) = map.get(&key) {
						return Err(serde::de::Error::custom(format!(
							"overlap on {key}; prev was {old:?}, current is {value:?}"
						)));
					}
					max_strokes = max_strokes.max(key.num_strokes());
					map.insert(key, value);
				}

				Ok(Dict { map, max_strokes })
			}
		}

		let visitor = MapVisitor {};
		deserializer.deserialize_map(visitor)
	}
}

impl Dict {
	pub fn load(path: &Path) -> Self {
		serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
	}

	pub fn get(&self, keys: &[Keys]) -> Option<&Entry> {
		self.map.get(keys)
	}

	pub fn max_strokes(&self) -> usize {
		self.max_strokes
	}
}
