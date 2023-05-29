use crate::bounded_queue::BoundedQueue;
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

/// XXX Use `floor_char_boundary` when stable.
fn floor_char_boundary_p(s: &str, index: usize) -> usize {
	if index >= s.len() {
		s.len()
	} else {
		let lower_bound = index.saturating_sub(3);
		let offset = (lower_bound..=index)
			.rev()
			.position(|index| s.is_char_boundary(index));

		index - offset.unwrap_or_else(|| unreachable!())
	}
}

impl Output {
	pub fn use_buffer(&mut self, buffer: &mut BoundedQueue<u8>) {
		self.diff_with_buffer(buffer);
		self.apply_to_buffer(buffer);
	}

	pub fn diff_with_buffer(&mut self, buffer: &mut BoundedQueue<u8>) {
		if self.delete_words != 0 {
			return;
		}

		let Some(buf_first_index) = buffer.len().checked_sub(self.delete.bytes()) else { return; };

		let same_bytes = buffer
			.inner()
			.range(buf_first_index..)
			.copied()
			.zip(self.append.bytes())
			.take_while(|(a, b)| a == b)
			.count();

		let same_bytes = floor_char_boundary_p(&self.append, same_bytes);
		let range = ..same_bytes;

		self.delete -= CharsOrBytes::for_str(&self.append[range]);
		self.append.drain(range);
	}

	pub fn apply_to_buffer(&self, buffer: &mut BoundedQueue<u8>) {
		if self.delete_words == 0 {
			for _ in 0..self.delete.bytes() {
				buffer.pop_back();
			}
		} else {
			buffer.clear();
		}

		for b in self.append.bytes() {
			buffer.push(b);
		}
	}
}
