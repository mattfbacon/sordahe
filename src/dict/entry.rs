use std::rc::Rc;
use std::str::FromStr;

use logos::Logos;
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

#[derive(Debug, PartialEq, Eq)]
pub enum Part {
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
	#[error("lexer error on {0:?}")]
	Lex(Box<str>),
	#[error(transparent)]
	PloverCommand(#[from] PloverCommandFromStrError),
	#[error(transparent)]
	Unescape(#[from] UnescapeError),
}

#[derive(Debug, Error)]
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

impl FromStr for Entry {
	type Err = ParseError;

	fn from_str(entry: &str) -> Result<Self, Self::Err> {
		#[derive(Logos)]
		enum EntryToken {
			#[regex(r"([^{]|\\\{)+")]
			Verbatim,
			#[regex(r"\{([^}\\]|\\.)*\}")]
			Special,
		}

		let mut ret = Vec::with_capacity(1);

		for (token, span) in EntryToken::lexer(entry).spanned() {
			match token.map_err(|_| ParseError::Lex(entry[span.clone()].into()))? {
				EntryToken::Verbatim => {
					let text = entry[span].trim();
					if !text.is_empty() {
						ret.push(Part::Verbatim(unescape(text)?));
					}
				}
				EntryToken::Special => {
					let inner = &entry[span.start + 1..span.end - 1];
					let part = match inner {
						"-|" => Part::SetCaps(true),
						">" => Part::SetCaps(false),
						"^" => Part::SetSpace(false),
						" " => Part::SetSpace(true),
						_ => {
							if let Some(command) = inner.strip_prefix("PLOVER:") {
								Part::PloverCommand(command.parse()?)
							} else if let Ok(punct) = inner.parse::<SpecialPunct>() {
								Part::SpecialPunct(punct)
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
									ret.push(Part::SetSpace(false));
								}
								if carry_to_next {
									ret.push(Part::CarryToNext);
								}
								if glue {
									ret.push(Part::Glue);
								}

								ret.push(Part::Verbatim(unescape(inner)?));

								if strip_after {
									ret.push(Part::SetSpace(false));
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
			Part::SetCaps(false),
			Part::Glue,
			Part::Verbatim("p^".into()),
		],
	);
}

#[derive(Clone, Debug, PartialEq, Eq, DeserializeFromStr)]
pub struct Entry(pub Rc<[Part]>);

impl From<Vec<Part>> for Entry {
	fn from(parts: Vec<Part>) -> Self {
		Self(parts.into())
	}
}
