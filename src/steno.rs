use std::collections::VecDeque;

use crate::dict::{Dict, Entry, EntryPart, PloverCommand, Strokes};
use crate::keys::{Key, Keys};

const BACKLOG_DEPTH: usize = 1000;

#[derive(Debug, Clone, Copy)]
struct InputState {
	caps: Option<bool>,
	space: bool,
	carry_to_next: bool,
	prev_was_glue: bool,
}

impl InputState {
	const INITIAL: Self = Self {
		caps: Some(true),
		space: false,
		carry_to_next: false,
		prev_was_glue: false,
	};
}

#[derive(Debug)]
struct InputEvent {
	strokes: Strokes,
	len: usize,
	state_before: InputState,
}

#[derive(Debug)]
pub struct Steno {
	dict: Dict,
	state: InputState,
	backlog: VecDeque<InputEvent>,
}

#[derive(Debug)]
struct Action {
	entry: Entry,
	strokes: Strokes,
	pop_backlog: usize,
	to_delete: u32,
	restore_state: Option<InputState>,
}

#[derive(Debug)]
pub enum Deletion {
	Word,
	Exact(u32),
}

#[derive(Debug)]
pub enum Output {
	Normal { delete: Deletion, append: String },
	Quit,
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

fn make_simple_action(text: Box<str>, keys: Keys) -> Action {
	Action {
		entry: vec![EntryPart::Verbatim(text)].into(),
		strokes: vec![keys].into(),
		pop_backlog: 0,
		to_delete: 0,
		restore_state: None,
	}
}

fn make_fallback_action(keys: Keys) -> Action {
	make_simple_action(keys.to_string().into(), keys)
}

impl Steno {
	pub fn new(dict: Dict) -> Self {
		Self {
			dict,
			state: InputState::INITIAL,
			backlog: VecDeque::with_capacity(BACKLOG_DEPTH),
		}
	}

	pub fn handle_keys(&mut self, keys: Keys) -> Output {
		let action = self.find_action(keys);
		self.run_action(action)
	}

	fn find_action(&self, this_keys: Keys) -> Action {
		if this_keys.contains(Key::NumberBar) {
			return make_numbers(this_keys).map_or_else(
				|| make_fallback_action(this_keys),
				|text| make_simple_action(text.into(), this_keys),
			);
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
					entry: entry.clone(),
					strokes: Strokes(all_strokes),
					pop_backlog: these_events.len(),
					to_delete: these_events
						.clone()
						.map(|event| event.len)
						.sum::<usize>()
						.try_into()
						.unwrap(),
					restore_state: event.map(|event| event.state_before),
				};
			}
			if let Some(event) = event {
				skip += event.strokes.num_strokes();
			}
		}

		make_fallback_action(this_keys)
	}

	fn run_part(&mut self, part: &EntryPart, buf: &mut String) -> Result<bool, PloverCommand> {
		let mut seen_glue = false;

		match part {
			EntryPart::Verbatim(text) => {
				if self.state.space {
					*buf += " ";
				}

				let first_pos = buf.len();
				*buf += text;

				if let Some(caps) = self.state.caps {
					let first_len = buf[first_pos..].chars().next().map_or(0, char::len_utf8);
					let first = &mut buf[first_pos..][..first_len];

					if caps {
						first.make_ascii_uppercase();
					} else {
						first.make_ascii_lowercase();
					}
				}

				if !std::mem::replace(&mut self.state.carry_to_next, false) {
					self.state.caps = None;
					self.state.space = true;
				}
			}
			EntryPart::SpecialPunct(punct) => {
				*buf += punct.as_str();
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
				if self.state.prev_was_glue {
					self.state.space = false;
					self.state.caps = None;
				}
				seen_glue = true;
			}
			EntryPart::PloverCommand(command) => return Err(*command),
		}
		Ok(seen_glue)
	}

	fn run_action(&mut self, action: Action) -> Output {
		self
			.backlog
			.truncate(self.backlog.len() - action.pop_backlog);

		if let Some(restore_state) = action.restore_state {
			self.state = restore_state;
		}

		let state_before = self.state;

		let mut buf = String::new();

		let mut seen_glue = false;
		for part in &*action.entry.0 {
			let res = self.run_part(part, &mut buf);
			match res {
				Ok(seen_glue_in_this) => {
					seen_glue |= seen_glue_in_this;
				}
				Err(PloverCommand::Backspace) => {
					assert!(buf.is_empty(), "cannot mix backspace with text");
					let prev = self.backlog.pop_back();
					if let Some(prev) = &prev {
						self.state = prev.state_before;
					}
					let delete = prev.map_or(Deletion::Word, |prev| {
						Deletion::Exact(prev.len.try_into().unwrap())
					});
					return Output::Normal {
						delete,
						append: String::new(),
					};
				}
				Err(PloverCommand::Quit) => {
					return Output::Quit;
				}
			}
		}
		self.state.prev_was_glue = seen_glue;

		self
			.backlog
			.drain(..self.backlog.len().saturating_sub(BACKLOG_DEPTH - 1));
		self.backlog.push_back(InputEvent {
			strokes: action.strokes,
			len: buf.len(),
			state_before,
		});

		Output::Normal {
			delete: Deletion::Exact(action.to_delete),
			append: buf,
		}
	}
}
