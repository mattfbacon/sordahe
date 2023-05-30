use super::{
	apply_orthography_rules, Action, Dict, InputEvent, InputState, SpecialAction, Steno, WordList,
};
use crate::chars_or_bytes::CharsOrBytes;
use crate::dict::{EntryPart, PloverCommand};

enum PreviousSource {
	InProgress,
	Backlog,
}

impl<D: Dict, W: WordList> Steno<D, W> {
	fn delete_full_entry(&mut self) -> Option<InputEvent> {
		let entry = self.backlog.pop_back();

		if let Some(entry) = &entry {
			self.state = entry.state_before;
			let delete = CharsOrBytes::for_str(&entry.text);
			self.output_in_progress.delete(delete);

			// When an entry is removed, `replaced_previous` indicates whether the previous entry's text should be re-appended.
			// However, if there are multiple strokes in this entry, then the "previous" entry is really a previous iteration of _this_ entry, so the _actual_ previous entry should not be re-appended.
			if entry.replaced_previous && entry.strokes.num_strokes() == 1 {
				if let Some(previous) = self.backlog.inner().back() {
					self.output_in_progress.append(&previous.text);
				}
			}
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
		let redo = &strokes[..strokes.len() - 1];
		for &stroke in redo {
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
	fn remove_previous(&mut self) -> Option<(String, PreviousSource)> {
		if let Some(text) = self.take_in_progress() {
			Some((text, PreviousSource::InProgress))
		} else if let Some(previous) = self.backlog.inner().back() {
			let text = &previous.text;
			self.output_in_progress.delete(CharsOrBytes::for_str(text));
			// XXX This clone is unfortunate and technically unnecessary.
			// If we could show Rust that this function only touches `*_in_progress` and `backlog`, we could return a `Cow` instead.
			// This could probably be accomplished by grouping the aforementioned fields into some kind of "inner" structure.
			Some((text.clone(), PreviousSource::Backlog))
		} else {
			None
		}
	}

	pub(in crate::steno) fn run_action(&mut self, action: Action) -> Result<(), SpecialAction> {
		assert!(self.backlog_entry_in_progress.is_empty());

		for _ in 0..action.delete_before {
			self.delete_full_entry();
		}

		let state_before = self.state;
		let mut replaced_previous = false;

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

					if let Some((previous_text, previous_source)) = previous {
						if matches!(previous_source, PreviousSource::Backlog) {
							replaced_previous = true;
						}

						let mut without_rules = [previous_text.as_str(), suffix].concat();
						without_rules.make_ascii_lowercase();
						if let Some(combined) = (!self.word_list.contains(without_rules.trim()))
							.then(|| apply_orthography_rules(&previous_text, suffix))
							.flatten()
						{
							self.run_verbatim(&combined);
						} else {
							self.run_verbatim(&previous_text);
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
				strokes: action.strokes,
				replaced_previous,
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
