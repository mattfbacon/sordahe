use std::collections::HashSet;
use std::convert::Infallible;
use std::path::Path;
use std::str::FromStr;

use anyhow::Context as _;

#[derive(Debug, Clone)]
pub struct WordList {
	words: HashSet<Box<str>>,
}

impl WordList {
	pub fn load(path: &Path) -> anyhow::Result<Self> {
		let raw = std::fs::read_to_string(path).with_context(|| format!("reading from {path:?}"))?;
		Ok(raw.parse().unwrap())
	}

	pub fn contains(&self, word: &str) -> bool {
		self.words.contains(word)
	}
}

impl FromStr for WordList {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let words = s.lines().map(Box::<str>::from).collect();
		Ok(Self { words })
	}
}
