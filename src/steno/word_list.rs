pub trait WordList {
	fn contains(&self, word: &str) -> bool;
}

// ...
impl AsRef<Self> for crate::word_list::WordList {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<T: AsRef<crate::word_list::WordList>> WordList for T {
	fn contains(&self, word: &str) -> bool {
		self.as_ref().contains(word)
	}
}
