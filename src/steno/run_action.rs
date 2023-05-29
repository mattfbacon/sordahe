use super::{
	apply_orthography_rules, Action, Dict, InputEvent, InputState, SpecialAction, Steno, WordList,
};
use crate::chars_or_bytes::CharsOrBytes;
use crate::dict::{EntryPart, PloverCommand};

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

	pub(in crate::steno) fn run_action(&mut self, action: Action) -> Result<(), SpecialAction> {
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
