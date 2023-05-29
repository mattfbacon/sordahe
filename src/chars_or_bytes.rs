use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Debug, Clone, Copy, Default)]
pub struct CharsOrBytes {
	chars: usize,
	bytes: usize,
}

impl CharsOrBytes {
	pub fn for_str(s: &str) -> Self {
		Self {
			chars: s.chars().count(),
			bytes: s.len(),
		}
	}

	pub fn chars(self) -> usize {
		self.chars
	}

	pub fn bytes(self) -> usize {
		self.bytes
	}
}

impl Add for CharsOrBytes {
	type Output = Self;

	fn add(self, rhs: Self) -> Self {
		Self {
			chars: self.chars + rhs.chars,
			bytes: self.bytes + rhs.bytes,
		}
	}
}

impl AddAssign for CharsOrBytes {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sub for CharsOrBytes {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self {
		Self {
			chars: self.chars - rhs.chars,
			bytes: self.bytes - rhs.bytes,
		}
	}
}

impl SubAssign for CharsOrBytes {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}
