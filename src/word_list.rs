use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct WordList {
	words: HashSet<Box<str>>,
}

impl WordList {
	pub fn load(path: &Path) -> Self {
		let raw = std::fs::read_to_string(path).unwrap();
		let words = raw.lines().map(Box::<str>::from).collect();
		Self { words }
	}

	pub fn contains(&self, word: &str) -> bool {
		self.words.contains(word)
	}
}
