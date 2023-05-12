use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

use logos::Logos as _;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_with::DeserializeFromStr;
use thiserror::Error;

use crate::keys::Keys;

#[derive(Debug, PartialEq, Eq)]
pub enum PloverCommand {
	Backspace,
}

impl std::str::FromStr for PloverCommand {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, ()> {
		Ok(match s {
			"backspace" => Self::Backspace,
			_ => return Err(()),
		})
	}
}

macro_rules! str_enum {
	($(#[$meta:meta])* $vis:vis enum $name:ident { $($variant:ident = $variant_str:tt),* $(,)? }) => {
		$(#[$meta])* $vis enum $name {
			$($variant,)*
		}

		impl std::str::FromStr for $name {
			type Err = ();

			fn from_str(s: &str) -> Result<Self, ()> {
				Ok(match s {
					$($variant_str => Self::$variant,)*
					_ => return Err(()),
				})
			}
		}

		impl $name {
			pub fn as_str(self) -> &'static str {
				match self {
					$(Self::$variant => $variant_str,)*
				}
			}
		}
	}
}

str_enum! {
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialPunct {
	Period = ".",
	Comma = ",",
	Colon = ":",
	Semi = ";",
	Bang = "!",
	Question = "?",
}
}

impl SpecialPunct {
	pub fn is_sentence_end(self) -> bool {
		match self {
			Self::Period | Self::Bang | Self::Question => true,
			Self::Colon | Self::Comma | Self::Semi => false,
		}
	}
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntryPart {
	Verbatim(Box<str>),
	SpecialPunct(SpecialPunct),
	SetCaps(bool),
	SetSpace(bool),
	CarryToNext,
	Glue,
	PloverCommand(PloverCommand),
}

#[derive(Debug, Error)]
pub enum ParseError {
	#[error("trailing dash")]
	TrailingDash,
	#[error("duplicate key(s) {0:?}")]
	Duplicate(Keys),
	#[error("unrecognized character {0:?}")]
	Unrecognized(char),
}

fn parse_part(part: &str) -> Result<Keys, ParseError> {
	let mut ret = Keys::empty();

	let mut prev_dash = false;

	macro_rules! do_double {
		($seen:ident, $first:ident, $second:ident) => {
			if prev_dash || ret.bits() >= Keys::$first.bits() {
				Keys::$second
			} else {
				Keys::$first
			}
		};
	}

	for ch in part.chars() {
		let new = match ch {
			'S' => do_double!(seen_s, S, S2),
			'T' => do_double!(seen_t, T, T2),
			'P' => do_double!(seen_p, P, P2),
			'H' => Keys::H,
			'*' => Keys::STAR,
			'F' => Keys::F,
			'L' => Keys::L,
			'D' => Keys::D,
			'K' => Keys::K,
			'W' => Keys::W,
			'R' => do_double!(seen_r, R, R2),
			'B' => Keys::B,
			'G' => Keys::G,
			'Z' => Keys::Z,
			'A' => Keys::A,
			'O' => Keys::O,
			'E' => Keys::E,
			'U' => Keys::U,
			'1' => Keys::NUMBER_BAR | Keys::S,
			'2' => Keys::NUMBER_BAR | Keys::T,
			'3' => Keys::NUMBER_BAR | Keys::P,
			'4' => Keys::NUMBER_BAR | Keys::H,
			'5' => Keys::NUMBER_BAR | Keys::A,
			'0' => Keys::NUMBER_BAR | Keys::O,
			'6' => Keys::NUMBER_BAR | Keys::F,
			'7' => Keys::NUMBER_BAR | Keys::P2,
			'8' => Keys::NUMBER_BAR | Keys::L,
			'9' => Keys::NUMBER_BAR | Keys::T2,
			'#' => Keys::NUMBER_BAR,
			'-' => {
				prev_dash = true;
				continue;
			}
			other => return Err(ParseError::Unrecognized(other)),
		};

		// Prevent duplicates, but ignore duplicates of the number bar.
		let overlap = ret & new & !Keys::NUMBER_BAR;
		if !overlap.is_empty() {
			return Err(ParseError::Duplicate(overlap));
		}

		// Note: `prev_dash` is intentionally ignored for characters without two keys.
		// This is compliant with the format of Plover's dictionary.
		prev_dash = false;
		ret |= new;
	}

	// Prevent trailing dash.
	if prev_dash {
		return Err(ParseError::TrailingDash);
	}

	Ok(ret)
}

#[test]
fn test_parse_part() {
	assert_eq!(parse_part("S").unwrap(), Keys::S);
	assert_eq!(parse_part("-S").unwrap(), Keys::S2);
	assert_eq!(parse_part("SS").unwrap(), Keys::S | Keys::S2);
	assert_eq!(parse_part("S-S").unwrap(), Keys::S | Keys::S2);
	// Respect steno order. This should not be `B | T`.
	assert_eq!(parse_part("BT").unwrap(), Keys::B | Keys::T2);
	assert_eq!(
		parse_part("AOEU").unwrap(),
		Keys::A | Keys::O | Keys::E | Keys::U
	);
	assert_eq!(
		parse_part("1234").unwrap(),
		Keys::NUMBER_BAR | Keys::S | Keys::T | Keys::P | Keys::H
	);
	assert_eq!(
		parse_part("#*EU").unwrap(),
		Keys::NUMBER_BAR | Keys::STAR | Keys::E | Keys::U,
	);
	assert_eq!(
		parse_part("1234ER78S").unwrap(),
		Keys::NUMBER_BAR
			| Keys::S
			| Keys::T
			| Keys::P
			| Keys::H
			| Keys::E
			| Keys::R2
			| Keys::P2
			| Keys::L
			| Keys::S2
	);
	assert_eq!(
		parse_part("1-RBGS").unwrap(),
		Keys::NUMBER_BAR | Keys::S | Keys::R2 | Keys::B | Keys::G | Keys::S2
	);
	assert_eq!(
		parse_part("KPA*BT").unwrap(),
		Keys::K | Keys::P | Keys::A | Keys::STAR | Keys::B | Keys::T2
	);
}

impl FromStr for Strokes {
	type Err = ParseError;

	fn from_str(raw: &str) -> Result<Self, ParseError> {
		let parts = raw.split('/');
		parts
			.map(parse_part)
			.collect::<Result<Vec<_>, _>>()
			.map(Self)
	}
}

#[derive(Debug, Error)]
pub enum EntryParseError {}

fn unescape(escaped: &str) -> Box<str> {
	let mut ret = String::with_capacity(escaped.len() / 2);

	let mut chars = escaped.chars();
	while let Some(ch) = chars.next() {
		ret.push(if ch == '\\' {
			let escape = chars.next().unwrap();
			match escape {
				'^' | '{' | '}' | '\\' => escape,
				_ => unreachable!("escape {escape:?}"),
			}
		} else {
			ch
		});
	}

	ret.into()
}

impl FromStr for Entry {
	type Err = EntryParseError;

	fn from_str(entry: &str) -> Result<Self, Self::Err> {
		#[derive(logos::Logos)]
		enum EntryToken {
			#[regex(r"([^{]|\\\{)+")]
			Verbatim,
			#[regex(r"\{([^}\\]|\\.)*\}")]
			Special,
		}

		let mut ret = Vec::with_capacity(1);

		for (token, span) in EntryToken::lexer(entry).spanned() {
			match token.expect(entry) {
				EntryToken::Verbatim => {
					let text = entry[span].trim();
					if !text.is_empty() {
						ret.push(EntryPart::Verbatim(unescape(text)));
					}
				}
				EntryToken::Special => {
					let inner = &entry[span.start + 1..span.end - 1];
					let part = match inner {
						"-|" => EntryPart::SetCaps(true),
						">" => EntryPart::SetCaps(false),
						"^" => EntryPart::SetSpace(false),
						" " => EntryPart::SetSpace(true),
						_ => {
							if let Some(command) = inner.strip_prefix("PLOVER:") {
								EntryPart::PloverCommand(command.parse().unwrap())
							} else if let Ok(punct) = inner.parse() {
								EntryPart::SpecialPunct(punct)
							} else {
								let (strip_before, inner) = inner
									.strip_prefix('^')
									.map_or((false, inner), |inner| (true, inner));
								let (carry_to_next, inner) = inner
									.strip_prefix("~|")
									.map_or((false, inner), |inner| (true, inner));
								let (glue, inner) = inner
									.strip_prefix('&')
									.map_or((false, inner), |inner| (true, inner));

								let (strip_after, inner) = inner
									.strip_suffix('^')
									.filter(|inner| !inner.ends_with('\\'))
									.map_or((false, inner), |inner| (true, inner));

								if !(strip_before || carry_to_next || glue || strip_after) {
									eprintln!("warn: pointless curlies around {inner:?}; treating as verbatim");
								}

								if strip_before {
									ret.push(EntryPart::SetSpace(false));
								}
								if carry_to_next {
									ret.push(EntryPart::CarryToNext);
								}
								if glue {
									ret.push(EntryPart::Glue);
								}

								ret.push(EntryPart::Verbatim(unescape(inner)));

								if strip_after {
									ret.push(EntryPart::SetSpace(false));
								}

								continue;
							}
						}
					};
					ret.push(part);
				}
			}
		}

		Ok(Self(ret.into()))
	}
}

#[test]
fn test_parse_entry() {
	assert_eq!(
		&r"{>} {&p\^}".parse::<Entry>().unwrap().0 as &[_],
		&[
			EntryPart::SetCaps(false),
			EntryPart::Glue,
			EntryPart::Verbatim("p^".into()),
		],
	);
}

#[derive(Clone, Debug, PartialEq, Eq, DeserializeFromStr)]
pub struct Entry(pub Rc<[EntryPart]>);

#[derive(Debug, PartialEq, Eq, Hash, DeserializeFromStr)]
pub struct Strokes(pub Vec<Keys>);

impl Strokes {
	pub fn num_strokes(&self) -> usize {
		self.0.len()
	}
}

impl Borrow<[Keys]> for Strokes {
	fn borrow(&self) -> &[Keys] {
		&self.0
	}
}

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

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("a string-to-string map")
			}

			fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
				let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));

				let mut max_strokes = 1;

				while let Some((key, value)) = access.next_entry::<Strokes, Entry>()? {
					if let Some(old) = map.get(&key) {
						panic!("overlap on {key:?}; prev was {old:?}, current is {value:?}");
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
	pub fn load() -> Self {
		serde_json::from_str(&std::fs::read_to_string("dict.json").unwrap()).unwrap()
	}

	pub fn get(&self, keys: &[Keys]) -> Option<&Entry> {
		self.map.get(keys)
	}

	pub fn max_strokes(&self) -> usize {
		self.max_strokes
	}
}
