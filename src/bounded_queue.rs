#![allow(dead_code /* API. */)]

use std::collections::{vec_deque, VecDeque};

#[derive(Debug)]
pub struct BoundedQueue<T> {
	inner: VecDeque<T>,
}

impl<T> BoundedQueue<T> {
	pub fn new(bound: usize) -> Self {
		Self {
			inner: VecDeque::with_capacity(bound),
		}
	}

	pub fn push(&mut self, item: T) {
		if self.inner.len() == self.inner.capacity() {
			self.inner.pop_front();
		}
		self.inner.push_back(item);
	}

	pub fn pop_back(&mut self) -> Option<T> {
		self.inner.pop_back()
	}

	pub fn clear(&mut self) {
		self.inner.clear();
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn inner(&self) -> &VecDeque<T> {
		&self.inner
	}

	pub fn last_n(&self, n: usize) -> vec_deque::Iter<'_, T> {
		self.inner.range(self.inner.len().saturating_sub(n)..)
	}
}
