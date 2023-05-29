use crate::chars_or_bytes::CharsOrBytes;

#[derive(Debug, Default)]
pub struct Output {
	pub delete_words: usize,
	pub delete: CharsOrBytes,
	pub append: String,
}

impl Output {
	pub(in crate::steno) fn delete(&mut self, amount: CharsOrBytes) {
		if amount.bytes() <= self.append.len() {
			self.append.truncate(self.append.len() - amount.bytes());
		} else {
			self.delete += amount - CharsOrBytes::for_str(&self.append);
			self.append.clear();
		}
	}

	pub(in crate::steno) fn delete_words(&mut self, words: usize) {
		// XXX check for text in the append buffer and possible delete it by word boundaries.
		assert!(self.append.is_empty());
		self.delete_words += words;
	}

	pub(in crate::steno) fn append(&mut self, text: &str) {
		self.append += text;
	}

	pub(in crate::steno) fn clear(&mut self) {
		self.append.clear();
		self.delete = CharsOrBytes::default();
		self.delete_words = 0;
	}
}
