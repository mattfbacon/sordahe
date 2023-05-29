use std::borrow::Borrow;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde_with::DeserializeFromStr;

use crate::keys::Keys;

#[derive(Debug, Default, PartialEq, Eq, Hash, DeserializeFromStr)]
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

impl FromStr for Strokes {
	type Err = crate::keys::ParseError;

	fn from_str(raw: &str) -> Result<Self, Self::Err> {
		let parts = raw.split('/');
		parts
			.map(Keys::from_str)
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
