use super::{Action, Dict, Steno, WordList};
use crate::dict::{Entry, EntryPart, Strokes};
use crate::keys::{Key, Keys};

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
	pub(in crate::steno) fn find_action(&self, this_keys: Keys) -> Action {
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
