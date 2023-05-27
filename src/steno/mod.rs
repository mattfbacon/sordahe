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
		}
	}

	pub fn handle_keys(&mut self, keys: Keys) -> Result<Output, SpecialAction> {
		let action = self.find_action(keys);
		self.run_action(action)
	}
}

// Implementation:

const BACKLOG_DEPTH: usize = 1000;

#[derive(Debug, Clone, Copy)]
struct InputState {
	caps: Option<bool>,
	space: bool,
	carry_to_next: bool,
	glue: bool,
}

impl InputState {
	const INITIAL: Self = Self {
		caps: Some(true),
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
					vec![EntryPart::Glue, EntryPart::Verbatim(text.into())]
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

struct OutputBuilder<'a> {
	backlog: &'a mut Backlog,
	output: Output,
	state_before: InputState,
	strokes: Strokes,
}

impl<'a> OutputBuilder<'a> {
	fn new(backlog: &'a mut Backlog, strokes: Strokes, input_state: InputState) -> Self {
		Self {
			backlog,
			output: Output::default(),
			state_before: input_state,
			strokes,
		}
	}

	fn delete_entry(&mut self, state: &mut InputState) -> Option<InputEvent> {
		let prev = self.backlog.pop_back();

		if let Some(prev) = &prev {
			*state = prev.state_before;
			self.state_before = prev.state_before;
			self.output.delete(CharsOrBytes::for_str(&prev.text));
		} else {
			self.output.delete_words(1);
		}

		prev
	}

	fn try_replace(
		&mut self,
		replacer: impl FnOnce(&str) -> Option<String>,
		state: &mut InputState,
	) -> bool {
		if !self.output.append.is_empty() {
			if let Some(replace) = replacer(&self.output.append) {
				self.output.append = replace;
				true
			} else {
				false
			}
		} else if let Some(last) = self.backlog.back_mut() {
			if let Some(replace) = replacer(&last.text) {
				self.delete_entry(state);
				self.output.append = replace;
				true
			} else {
				false
			}
		} else {
			false
		}
	}

	fn append(&mut self, text: &str) {
		self.output.append(text);
	}

	fn append_capsed(&mut self, text: &str, caps: Option<bool>) {
		let first_pos = self.output.append.len();

		self.output.append(text);
		let text = &mut self.output.append[first_pos..];

		if let Some(caps) = caps {
			let first_len = text.chars().next().map_or(0, char::len_utf8);
			let first = &mut text[..first_len];

			if caps {
				first.make_ascii_uppercase();
			} else {
				first.make_ascii_lowercase();
			}
		}
	}

	fn finish(self) -> Output {
		while self.backlog.len() >= BACKLOG_DEPTH {
			self.backlog.pop_front();
		}

		if !self.output.append.is_empty() {
			self.backlog.push_back(InputEvent {
				strokes: self.strokes,
				text: self.output.append.clone(),
				state_before: self.state_before,
			});
		}

		self.output
	}
}

impl<D: Dict, W: WordList> Steno<D, W> {
	fn run_action(&mut self, action: Action) -> Result<Output, SpecialAction> {
		let mut backlog = std::mem::take(&mut self.backlog);
		let mut output = OutputBuilder::new(&mut backlog, action.strokes, self.state);

		for _ in 0..action.delete_before {
			output.delete_entry(&mut self.state);
		}

		let glued = self.state.glue;
		self.state.glue = false;

		for part in &*action.entry.0 {
			match self.run_part(part, &mut output, glued) {
				Ok(()) => {}
				Err(PloverCommand::Backspace) => {
					output.delete_entry(&mut self.state);
				}
				Err(PloverCommand::Quit) => return Err(SpecialAction::Quit),
			}
		}

		let output = output.finish();
		self.backlog = backlog;
		Ok(output)
	}

	fn run_part(
		&mut self,
		part: &EntryPart,
		output: &mut OutputBuilder<'_>,
		prev_was_glue: bool,
	) -> Result<(), PloverCommand> {
		match part {
			EntryPart::Verbatim(text) => {
				if self.state.space {
					output.append(" ");
				}

				let mut already_appended = false;

				if !self.state.space {
					already_appended = output.try_replace(
						|before| {
							let mut without_rules = [before, text].concat();
							without_rules.make_ascii_lowercase();
							if self.word_list.contains(&without_rules) {
								return None;
							}

							apply_orthography_rules(before, text)
						},
						&mut self.state,
					);
				}

				if !already_appended {
					output.append_capsed(text, self.state.caps);
				}

				if !std::mem::replace(&mut self.state.carry_to_next, false) {
					self.state.caps = None;
					self.state.space = true;
				}
			}
			EntryPart::SpecialPunct(punct) => {
				output.append(punct.as_str());
				self.state.space = true;
				self.state.caps = if punct.is_sentence_end() {
					Some(true)
				} else {
					None
				};
			}
			EntryPart::SetCaps(set) => {
				self.state.caps = Some(*set);
			}
			EntryPart::SetSpace(set) => {
				self.state.space = *set;
			}
			EntryPart::CarryToNext => {
				self.state.carry_to_next = true;
			}
			EntryPart::Glue => {
				if prev_was_glue {
					self.state.space = false;
					self.state.caps = None;
				}
				self.state.glue = true;
			}
			EntryPart::PloverCommand(command) => return Err(*command),
		}

		Ok(())
	}
}
