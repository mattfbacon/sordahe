use std::collections::HashMap;

use logos::Logos as _;

use crate::keys::Keys;

#[derive(Debug, PartialEq, Eq)]
pub enum PloverCommand {}

impl std::str::FromStr for PloverCommand {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, ()> {
		match s {
			_ => Err(()),
		}
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
	Glue,
	PloverCommand(PloverCommand),
}

pub type Entry = Vec<EntryPart>;

#[derive(Debug)]
pub struct Dict(HashMap<Vec<Keys>, Entry>);

type Raw = HashMap<Box<str>, Box<str>>;

#[derive(Debug)]
enum ParseError {
	TrailingDash,
	Duplicate(Keys),
	Unrecognized(char),
}

fn parse_part(part: &str) -> Result<Keys, ParseError> {
	let mut ret = Keys::empty();

	let mut seen_s = false;
	let mut seen_t = false;
	let mut seen_p = false;
	let mut seen_r = false;

	let mut prev_dash = false;

	macro_rules! do_double {
		($seen:ident, $first:ident, $second:ident) => {
			if $seen || prev_dash {
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

		macro_rules! set_seen {
			($seen:ident, $first:ident, $second:ident) => {
				if new.contains(Keys::$first) || new.contains(Keys::$second) {
					$seen = true;
				}
			};
		}

		set_seen!(seen_s, S, S2);
		set_seen!(seen_t, T, T2);
		set_seen!(seen_p, P, P2);
		set_seen!(seen_r, R, R2);

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
			| Keys::P2
			| Keys::L
			| Keys::R
			| Keys::S2
			| Keys::E
	);
	assert_eq!(
		parse_part("1-RBGS").unwrap(),
		Keys::NUMBER_BAR | Keys::S | Keys::R2 | Keys::B | Keys::G | Keys::S2
	);
	assert_eq!(
		parse_part("KPA*BT").unwrap(),
		Keys::K | Keys::P | Keys::A | Keys::STAR | Keys::B | Keys::T
	);
}

fn parse_keys_str(keys: &str) -> Result<Vec<Keys>, ParseError> {
	let parts = keys.split('/');
	parts.map(parse_part).collect()
}

#[derive(logos::Logos)]
enum EntryToken {
	#[regex(r"([^{]|\\\{)+")]
	Verbatim,
	#[regex(r"\{([^}\\]|\\.)*\}")]
	Special,
}

fn parse_entry(entry: &str) -> Entry {
	let mut ret = Vec::with_capacity(1);

	for (token, span) in EntryToken::lexer(entry).spanned() {
		let part = match token.expect(entry) {
			EntryToken::Verbatim => EntryPart::Verbatim(entry[span].trim().into()),
			EntryToken::Special => {
				let inner = &entry[span.start + 1..span.end - 1];
				match inner {
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
							let (glue, inner) = inner
								.strip_prefix('&')
								.map_or((false, inner), |inner| (true, inner));
							let (strip_after, inner) = inner
								.strip_suffix('^')
								.filter(|inner| !inner.ends_with('\\'))
								.map_or((false, inner), |inner| (true, inner));

							if !(strip_before || strip_after || glue) {
								eprintln!("warn: pointless curlies around {inner:?}; treating as verbatim");
							}

							if strip_before {
								ret.push(EntryPart::SetSpace(false));
							}
							if glue {
								ret.push(EntryPart::Glue);
							}

							ret.push(EntryPart::Verbatim(inner.into()));

							if strip_after {
								ret.push(EntryPart::SetSpace(false));
							}

							continue;
						}
					}
				}
			}
		};
		ret.push(part);
	}

	ret
}

#[test]
fn test_parse_entry() {
	assert_eq!(
		parse_entry("{>}{&p}"),
		vec![
			EntryPart::SetCaps(false),
			EntryPart::Glue,
			EntryPart::Verbatim("p".into()),
		]
	);
}

impl Dict {
	pub fn load() -> Self {
		let raw: Raw = serde_json::from_str(&std::fs::read_to_string("dict.json").unwrap()).unwrap();
		let mut map = HashMap::with_capacity(raw.len());
		for (k, v) in raw
			.into_iter()
			.map(|(keys, output)| (parse_keys_str(&keys).expect(&keys), parse_entry(&output)))
		{
			if let Some(old) = map.get(&k) {
				eprintln!("warn: overlap on {k:?}; prev was {old:?}, current is {v:?}");
			}
			map.insert(k, v);
		}
		Self(map)
	}

	pub fn get(&self, keys: &[Keys]) -> Option<&[EntryPart]> {
		self.0.get(keys).map(|s| &**s)
	}
}
