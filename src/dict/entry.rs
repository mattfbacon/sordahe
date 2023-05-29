use std::ops::Add;
use std::str::FromStr;
use std::sync::Arc;

use paste::paste;
use serde_with::DeserializeFromStr;
use thiserror::Error;

macro_rules! str_enum {
	(#[description = $descr:tt] $(#[$meta:meta])* $vis:vis enum $name:ident { $($variant:ident = $variant_str:tt),* $(,)? }) => { paste! {
		$(#[$meta])* $vis enum $name {
			$($variant,)*
		}

		#[derive(Debug, Error)]
		#[error("unrecognized {} {0:?}", Self::DESCRIPTION)]
		pub struct [<$name FromStrError>](Box<str>);

		impl [<$name FromStrError>] {
			const DESCRIPTION: &str = $descr;
		}

		impl FromStr for $name {
			type Err = [<$name FromStrError>];

			fn from_str(s: &str) -> Result<Self, Self::Err> {
				Ok(match s {
					$($variant_str => Self::$variant,)*
					_ => return Err([<$name FromStrError>](s.into())),
				})
			}
		}

		impl $name {
			#[allow(dead_code)]
			pub fn as_str(self) -> &'static str {
				match self {
					$(Self::$variant => $variant_str,)*
				}
			}
		}
	} }
}

str_enum! {
#[description = "plover command"]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PloverCommand {
	Backspace = "backspace",
	Quit = "quit",
	Reset = "reset",
}
}

str_enum! {
#[description = "special punct"]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
	Verbatim(Box<str>),
	Suffix(Box<str>),
	Glue(Box<str>),

	SpecialPunct(SpecialPunct),

	SetCaps(bool),
	SetSpace(bool),
	CarryToNext,

	PloverCommand(PloverCommand),
}

#[derive(Debug, Error)]
pub enum ParseError {
	#[error("unclosed bracket")]
	UnclosedBracket,
	#[error("pointless brackets around {0:?}")]
	PointlessBrackets(Box<str>),
	#[error(transparent)]
	PloverCommand(#[from] PloverCommandFromStrError),
	#[error(transparent)]
	Unescape(#[from] UnescapeError),
}

#[derive(Clone, Copy, Debug, Error)]
pub enum UnescapeError {
	#[error("unexpected EOF after backslash; expected escape")]
	UnexpectedEof,
	#[error("unknown escape {0:?}")]
	UnknownEscape(char),
}

fn unescape(escaped: &str) -> Result<Box<str>, UnescapeError> {
	let mut ret = String::with_capacity(escaped.len() / 2);

	let mut chars = escaped.chars();
	while let Some(ch) = chars.next() {
		ret.push(if ch == '\\' {
			let escape = chars.next().ok_or(UnescapeError::UnexpectedEof)?;
			match escape {
				'^' | '{' | '}' | '\\' => escape,
				_ => return Err(UnescapeError::UnknownEscape(escape)),
			}
		} else {
			ch
		});
	}

	Ok(ret.into())
}

trait StrExt {
	fn find_with_escapes(&self, pattern: char) -> Option<usize>;
}

impl StrExt for str {
	fn find_with_escapes(&self, pattern: char) -> Option<usize> {
		debug_assert!(pattern != '\\');
		let mut escape = false;
		for (idx, ch) in self.char_indices() {
			if escape {
				escape = false;
			} else if ch == '\\' {
				escape = true;
			} else if ch == pattern {
				return Some(idx);
			}
		}
		None
	}
}

#[test]
fn test_find_unescaped() {
	assert_eq!("abc".find_with_escapes('b'), Some(1));
	assert_eq!(r"a\bc".find_with_escapes('b'), None);
	assert_eq!(r"a\\bc".find_with_escapes('b'), Some(3));
}

macro_rules! push_verbatim {
	($out:expr, $s:expr) => {{
		let s = $s;
		if !s.is_empty() {
			$out.push(Part::Verbatim(unescape(s)?));
		}
	}};
}

fn parse_special(out: &mut Vec<Part>, inner: &str) -> Result<(), ParseError> {
	'singles: {
		let part = match inner {
			">" => Part::SetCaps(false),
			"-|" => Part::SetCaps(true),
			"^" => Part::SetSpace(false),
			" " => Part::Verbatim(" ".into()),
			"~|" => Part::CarryToNext,
			_ => break 'singles,
		};
		out.push(part);
		return Ok(());
	}

	'precheck: {
		let part = if let Some(command) = inner.strip_prefix("PLOVER:") {
			Part::PloverCommand(command.parse()?)
		} else if let Ok(punct) = inner.parse::<SpecialPunct>() {
			Part::SpecialPunct(punct)
		} else {
			break 'precheck;
		};

		out.push(part);
		return Ok(());
	}

	if let Some(glued) = inner.strip_prefix('&') {
		out.push(Part::Glue(unescape(glued)?));
		return Ok(());
	}

	let (inner, set_space_false_after) = inner
		.strip_suffix('^')
		.filter(|rest| rest.bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 0)
		.map_or((inner, false), |inner| (inner, true));

	out.push(if let Some(suffix) = inner.strip_prefix('^') {
		Part::Suffix(unescape(suffix)?)
	} else {
		Part::Verbatim(unescape(inner)?)
	});

	if set_space_false_after {
		out.push(Part::SetSpace(false));
	}

	Ok(())
}

impl FromStr for Entry {
	type Err = ParseError;

	fn from_str(entry: &str) -> Result<Self, Self::Err> {
		let mut out = Vec::with_capacity(1);

		let mut rest = entry;

		while let Some(special_start) = rest.find_with_escapes('{') {
			let before = &rest[..special_start];
			push_verbatim!(out, before.trim());

			rest = &rest[special_start + 1..];
			let special_end = rest
				.find_with_escapes('}')
				.ok_or(ParseError::UnclosedBracket)?;
			let special = &rest[..special_end];
			rest = &rest[special_end + 1..];

			parse_special(&mut out, special)?;
		}

		push_verbatim!(out, rest.trim());

		Ok(Self(out.into()))
	}
}

#[test]
fn test_parse_entry() {
	assert_eq!(
		&r"\{{>}\} {&p\^} abc".parse::<Entry>().unwrap().0 as &[_],
		&[
			Part::Verbatim("{".into()),
			Part::SetCaps(false),
			Part::Verbatim("}".into()),
			Part::Glue("p^".into()),
			Part::Verbatim("abc".into()),
		],
	);
	assert_eq!(
		&r"{^ ^}".parse::<Entry>().unwrap().0 as &[_],
		&[Part::Suffix(" ".into()), Part::SetSpace(false)],
	);
	assert_eq!(
		&r"{\\^}".parse::<Entry>().unwrap().0 as &[_],
		&[Part::Verbatim("\\".into()), Part::SetSpace(false)],
	);
	assert_eq!(
		&r"{^\\\\\^}".parse::<Entry>().unwrap().0 as &[_],
		&[Part::Suffix(r"\\^".into())],
	);
}

#[derive(Clone, Debug, PartialEq, Eq, DeserializeFromStr)]
pub struct Entry(pub Arc<[Part]>);

impl From<Vec<Part>> for Entry {
	fn from(parts: Vec<Part>) -> Self {
		Self(parts.into())
	}
}

impl Add<&Entry> for &Entry {
	type Output = Entry;

	fn add(self, other: &Entry) -> Entry {
		self
			.0
			.iter()
			.cloned()
			.chain(other.0.iter().cloned())
			.collect::<Vec<_>>()
			.into()
	}
}
