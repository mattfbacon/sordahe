use self::chars_or_bytes::CharsOrBytes;
use self::orthography::apply_orthography_rules;
use crate::bounded_queue::BoundedQueue;
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
			backlog: BoundedQueue::new(BACKLOG_DEPTH),

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

#[allow(clippy::struct_excessive_bools /* No Clippy, it's not a state machine, I promise. */)]
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

type Backlog = BoundedQueue<InputEvent>;

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
	/// If `Some`, this should be run after `entry`.
	removed_suffix: Option<Entry>,
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
		removed_suffix: None,
		delete_before: 0,
	}
}

fn make_fallback_action(keys: Keys) -> Action {
	make_text_action(keys.to_string().into(), keys)
}

fn split_suffix(keys: Keys) -> Option<(Keys, Keys)> {
	let suffix_keys = Key::G | Key::S2 | Key::D | Key::Z;

	let suffix = keys & suffix_keys;
	(!suffix.is_empty()).then(|| (keys & !suffix_keys, suffix))
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

		let split_suffix = split_suffix(this_keys)
			.and_then(|(without_suffix, suffix)| Some((without_suffix, self.dict.get(&[suffix])?)));

		// As a by-reference iterator, this is cheaply cloneable, which we take advantage of.
		let events = self.backlog.last_n(max_strokes);

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
			let these_strokes = &mut all_strokes[skip..];

			if let Some(entry) = self.dict.get(these_strokes) {
				all_strokes.drain(..skip);
				return Action {
					entry,
					strokes: Strokes(all_strokes),
					removed_suffix: None,
					delete_before: these_events.len(),
				};
			}

			if let Some((without_suffix, suffix)) = &split_suffix {
				*these_strokes.last_mut().unwrap() = *without_suffix;
				let entry = self.dict.get(these_strokes);
				*these_strokes.last_mut().unwrap() = this_keys;

				if let Some(entry) = entry {
					all_strokes.drain(..skip);
					return Action {
						entry,
						strokes: Strokes(all_strokes),
						removed_suffix: Some(suffix.clone()),
						delete_before: these_events.len(),
					};
				}
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

	fn clear(&mut self) {
		self.append.clear();
		self.delete = CharsOrBytes::default();
		self.delete_words = 0;
	}
}

enum PreviousSource {
	InProgress(String),
	Backlog(InputEvent),
}

impl PreviousSource {
	fn text(&self) -> &str {
		match self {
			Self::InProgress(text) => text,
			Self::Backlog(event) => &event.text,
		}
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

	fn take_in_progress(&mut self) -> Option<String> {
		if self.backlog_entry_in_progress.is_empty() {
			return None;
		}

		let text = std::mem::take(&mut self.backlog_entry_in_progress);
		self.output_in_progress.delete(CharsOrBytes::for_str(&text));
		Some(text)
	}

	#[allow(clippy::manual_map /* symmetry */)]
	fn remove_previous(&mut self) -> Option<PreviousSource> {
		if let Some(text) = self.take_in_progress() {
			Some(PreviousSource::InProgress(text))
		} else if let Some(previous) = self.delete_full_entry() {
			Some(PreviousSource::Backlog(previous))
		} else {
			None
		}
	}

	fn run_action(&mut self, action: Action) -> Result<(), SpecialAction> {
		assert!(self.backlog_entry_in_progress.is_empty());

		for _ in 0..action.delete_before {
			self.delete_full_entry();
		}

		let mut state_before = self.state;
		let mut strokes = action.strokes;

		let suffix_parts = action
			.removed_suffix
			.as_ref()
			.into_iter()
			.flat_map(|entry| &*entry.0);
		for part in action.entry.0.iter().chain(suffix_parts) {
			match part {
				EntryPart::Verbatim(text) => {
					self.run_verbatim(text);
				}
				EntryPart::Suffix(suffix) => {
					let previous = self.remove_previous();
					self.state.space = false;

					if let Some(mut previous) = previous {
						if let PreviousSource::Backlog(event) = &mut previous {
							state_before = event.state_before;
							// This weird dance lets us avoid inserting at the front.
							event.strokes.0.extend_from_slice(&strokes.0);
							strokes = std::mem::take(&mut event.strokes);
						}
						let previous_text = previous.text();

						let mut without_rules = [previous_text, suffix].concat();
						without_rules.make_ascii_lowercase();
						if let Some(combined) = (!self.word_list.contains(without_rules.trim()))
							.then(|| apply_orthography_rules(previous_text, suffix))
							.flatten()
						{
							self.run_verbatim(&combined);
						} else {
							self.run_verbatim(previous_text);
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
					PloverCommand::Reset => {
						self.state = InputState::INITIAL;
						self.backlog.clear();
						self.backlog_entry_in_progress.clear();
						self.output_in_progress.clear();
					}
				},
			}
		}

		if !self.backlog_entry_in_progress.is_empty() {
			// Not using `std::mem::take` here because we want to retain the allocated buffer for future pushes.
			let text = self.backlog_entry_in_progress.clone();
			self.backlog_entry_in_progress.clear();
			self.backlog.push(InputEvent {
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
