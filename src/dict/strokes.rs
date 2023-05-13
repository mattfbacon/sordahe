use std::borrow::Borrow;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde_with::DeserializeFromStr;
use thiserror::Error;

use crate::keys::Keys;

#[derive(Debug, PartialEq, Eq, Hash, DeserializeFromStr)]
pub struct Strokes(pub Vec<Keys>);

impl Strokes {
	pub fn num_strokes(&self) -> usize {
		self.0.len()
	}
}

impl From<Vec<Keys>> for Strokes {
	fn from(keys: Vec<Keys>) -> Self {
		Self(keys)
	}
}

impl Borrow<[Keys]> for Strokes {
	fn borrow(&self) -> &[Keys] {
		&self.0
	}
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

impl Display for Strokes {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		let [first, rest @ ..] = self.0.as_slice() else { return Ok(()); };
		first.fmt(formatter)?;
		for keys in rest {
			formatter.write_str("/")?;
			keys.fmt(formatter)?;
		}
		Ok(())
	}
}
