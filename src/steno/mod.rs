use std::collections::VecDeque;

use self::chars_or_bytes::CharsOrBytes;
use self::orthography::apply_orthography_rules;
use crate::dict::{Entry, EntryPart, PloverCommand, Strokes};
use crate::keys::{Key, Keys};

mod chars_or_bytes;
mod orthography;
#[cfg(test)]
mod test;

// Public API:

pub trait Dict {
	fn get(&self, keys: &[Keys]) -> Option<Entry>;
	fn max_strokes(&self) -> usize;
}

// ...
impl AsRef<Self> for crate::dict::Dict {
	fn as_ref(&self) -> &Self {
		self
	}
}

impl<T: AsRef<crate::dict::Dict>> Dict for T {
	fn get(&self, keys: &[Keys]) -> Option<Entry> {
		// Cheap ref-counted clone.
		self.as_ref().get(keys).cloned()
	}

	fn max_strokes(&self) -> usize {
		self.as_ref().max_strokes()
	}
}

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

#[derive(Debug)]
pub enum SpecialAction {
	Quit,
}

#[derive(Debug, Default)]
pub struct Output {
	pub delete_words: usize,
	pub delete: CharsOrBytes,
	pub append: String,
}

impl<D: Dict, W: WordList> Steno<D, W> {
	pub fn new(dict: D, word_list: W) -> Self {
		Self {
			dict,
			word_list,
			state: InputState::INITIAL,
			backlog: VecDeque::with_capacity(BACKLOG_DEPTH),

			output_in_progress: Output::default(),
			backlog_entry_in_progress: String::new(),
		}
	}

	pub fn run_keys(&mut self, keys: Keys) -> Result<(), SpecialAction> {
		let action = self.find_action(keys);
		self.run_action(action)
	}

	pub fn flush(&mut self) -> Output {
		std::mem::take(&mut self.output_in_progress)
	}
}

// Implementation:

const BACKLOG_DEPTH: usize = 1000;

#[derive(Debug, Clone, Copy)]
struct InputState {
	caps: bool,
	space: bool,
	carry_to_next: bool,
	glue: bool,
}

impl InputState {
	const INITIAL: Self = Self {
		caps: true,
		space: false,
		carry_to_next: false,
		glue: false,
	};
}

#[derive(Debug)]
struct InputEvent {
	strokes: Strokes,
	text: String,
	state_before: InputState,
}

type Backlog = VecDeque<InputEvent>;

#[derive(Debug)]
pub struct Steno<D = crate::dict::Dict, W = crate::word_list::WordList> {
	dict: D,
	word_list: W,
	state: InputState,
	backlog: Backlog,

	output_in_progress: Output,
	backlog_entry_in_progress: String,
}

#[derive(Debug)]
struct Action {
	entry: Entry,
	strokes: Strokes,
	/// The number of backlog entries that must be deleted before applying the entry.
	delete_before: usize,
}

fn make_numbers(mut keys: Keys) -> Option<String> {
	const NUMBERS: &[(Key, u8)] = &[
		(Key::S, b'1'),
		(Key::T, b'2'),
		(Key::P, b'3'),
		(Key::H, b'4'),
		(Key::A, b'5'),
		(Key::O, b'0'),
		(Key::F, b'6'),
		(Key::P2, b'7'),
		(Key::L, b'8'),
		(Key::T2, b'9'),
	];

	keys.remove(Key::NumberBar);

	let mut ret = Vec::new();

	for &(key, ch) in NUMBERS {
		if keys.remove(key) {
			ret.push(ch);
		}
	}

	// Make sure there is actually at least one number.
	if ret.is_empty() {
		return None;
	}

	if keys.remove(Key::E | Key::U) {
		ret.reverse();
	}

	if keys.remove(Key::D | Key::Z) {
		ret.insert(0, b'$');
		ret.extend_from_slice(b"00");
	} else {
		if keys.remove(Key::D) {
			ret.extend_from_within(..);
		}
		if keys.remove(Key::Z) {
			ret.extend_from_slice(b"00");
		}
	}

	if keys.remove(Key::K) || keys.remove(Key::B | Key::G) {
		ret.extend_from_slice(b":00");
	}

	if !keys.is_empty() {
		return None;
	}

	Some(String::from_utf8(ret).unwrap())
}

fn make_text_action(text: Box<str>, keys: Keys) -> Action {
	make_simple_action(vec![EntryPart::Verbatim(text)].into(), keys)
}

fn make_simple_action(entry: Entry, keys: Keys) -> Action {
	Action {
		entry,
		strokes: vec![keys].into(),
		delete_before: 0,
	}
}

fn make_fallback_action(keys: Keys) -> Action {
	make_text_action(keys.to_string().into(), keys)
}

impl<D: Dict, W: WordList> Steno<D, W> {
	fn find_action(&self, this_keys: Keys) -> Action {
		if this_keys.contains(Key::NumberBar) {
			if let Some(text) = make_numbers(this_keys) {
				let entry = if text.bytes().all(|b| b.is_ascii_digit()) {
					vec![EntryPart::Glue(text.into())]
				} else {
					vec![EntryPart::Verbatim(text.into())]
				};

				return make_simple_action(entry.into(), this_keys);
			}
		}

		let max_strokes = self.dict.max_strokes();

		// As a by-reference iterator, this is cheaply cloneable, which we take advantage of.
		let events = self
			.backlog
			.range(self.backlog.len().saturating_sub(max_strokes)..);

		let mut all_strokes: Vec<Keys> = events
			.clone()
			.flat_map(|event| &event.strokes.0)
			.copied()
			.chain(std::iter::once(this_keys))
			.collect();

		let mut skip = 0;
		for (i, event) in events
			.clone()
			.map(Some)
			.chain(std::iter::once(None))
			.enumerate()
		{
			let these_events = events.clone().skip(i);
			let these_strokes = &all_strokes[skip..];
			if let Some(entry) = self.dict.get(these_strokes) {
				all_strokes.drain(..skip);
				return Action {
					entry,
					strokes: Strokes(all_strokes),
					delete_before: these_events.len(),
				};
			}
			if let Some(event) = event {
				skip += event.strokes.num_strokes();
			}
		}

		make_fallback_action(this_keys)
	}
}

impl Output {
	fn delete(&mut self, amount: CharsOrBytes) {
		if amount.bytes() <= self.append.len() {
			self.append.truncate(self.append.len() - amount.bytes());
		} else {
			self.delete += amount - CharsOrBytes::for_str(&self.append);
			self.append.clear();
		}
	}

	fn delete_words(&mut self, words: usize) {
		// XXX check for text in the append buffer and possible delete it by word boundaries.
		assert!(self.append.is_empty());
		self.delete_words += words;
	}

	fn append(&mut self, text: &str) {
		self.append += text;
	}
}

impl<D: Dict, W: WordList> Steno<D, W> {
	fn delete_full_entry(&mut self) -> Option<InputEvent> {
		let entry = self.backlog.pop_back();

		if let Some(entry) = &entry {
			self.state = entry.state_before;
			let delete = CharsOrBytes::for_str(&entry.text);
			self.output_in_progress.delete(delete);
		} else {
			self.output_in_progress.delete_words(1);
		}

		entry
	}

	fn undo_stroke(&mut self) -> Result<(), SpecialAction> {
		// First delete an entire entry.
		let Some(entry) = self.delete_full_entry() else { return Ok(()); };
		let strokes = entry.strokes.0;

		// Then re-run all but the last stroke.
		for &stroke in &strokes[..strokes.len() - 1] {
			self.run_keys(stroke)?;
		}

		Ok(())
	}

	fn run_action(&mut self, action: Action) -> Result<(), SpecialAction> {
		assert!(self.backlog_entry_in_progress.is_empty());

		for _ in 0..action.delete_before {
			self.delete_full_entry();
		}

		let mut state_before = self.state;
		let mut strokes = action.strokes;

		for part in &*action.entry.0 {
			match part {
				EntryPart::Verbatim(text) => {
					self.run_verbatim(text);
				}
				EntryPart::Suffix(suffix) => {
					assert!(self.backlog_entry_in_progress.is_empty());
					let previous = self.delete_full_entry();
					self.state.space = false;

					if let Some(mut previous) = previous {
						state_before = previous.state_before;
						previous.strokes.0.extend_from_slice(&strokes.0);
						strokes = previous.strokes;

						let mut without_rules = [previous.text.as_str(), suffix].concat();
						without_rules.make_ascii_lowercase();
						if let Some(combined) = (!self.word_list.contains(without_rules.trim()))
							.then(|| apply_orthography_rules(&previous.text, suffix))
							.flatten()
						{
							self.run_verbatim(&combined);
						} else {
							self.run_verbatim(&previous.text);
							self.append(suffix);
						}
					} else {
						self.run_verbatim(suffix);
					}
				}
				EntryPart::SpecialPunct(punct) => {
					self.append(punct.as_str());
					self.state.space = true;
					self.state.caps = punct.is_sentence_end();
				}
				EntryPart::SetCaps(set) => {
					self.state.caps = *set;
				}
				EntryPart::SetSpace(set) => {
					self.state.space = *set;
				}
				EntryPart::CarryToNext => {
					self.state.carry_to_next = true;
				}
				EntryPart::Glue(glued) => {
					if self.state.glue {
						self.append(glued);
					} else {
						self.run_verbatim(glued);
					}
					self.state.glue = true;
				}
				EntryPart::PloverCommand(command) => match command {
					PloverCommand::Backspace => {
						assert!(self.backlog_entry_in_progress.is_empty());
						self.undo_stroke()?;
					}
					PloverCommand::Quit => return Err(SpecialAction::Quit),
				},
			}
		}

		if !self.backlog_entry_in_progress.is_empty() {
			while self.backlog.len() >= BACKLOG_DEPTH {
				self.backlog.pop_front();
			}
			// Not using `std::mem::take` here because we want to retain the allocated buffer for future pushes.
			let text = self.backlog_entry_in_progress.clone();
			self.backlog_entry_in_progress.clear();
			self.backlog.push_back(InputEvent {
				strokes,
				text,
				state_before,
			});
		}

		Ok(())
	}

	fn append(&mut self, text: &str) {
		self.output_in_progress.append(text);
		self.backlog_entry_in_progress += text;
	}

	fn append_capsed(&mut self, text: &str, caps: bool) {
		let pos = self.output_in_progress.append.len();

		self.output_in_progress.append(text);
		let text = &mut self.output_in_progress.append[pos..];

		if caps {
			let first_len = text.chars().next().map_or(0, char::len_utf8);
			let first_text = &mut text[..first_len];
			first_text.make_ascii_uppercase();
		}

		self.backlog_entry_in_progress += text;
	}

	fn run_verbatim(&mut self, text: &str) {
		self.state.glue = false;

		if self.state.space {
			self.append(" ");
		}

		self.append_capsed(text, self.state.caps);

		if !std::mem::replace(&mut self.state.carry_to_next, false) {
			self.state.caps = false;
			self.state.space = true;
		}
	}
}
