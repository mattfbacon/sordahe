pub use self::dict::Dict;
use self::orthography::apply_orthography_rules;
pub use self::output::Output;
pub use self::word_list::WordList;
use crate::bounded_queue::BoundedQueue;
use crate::dict::{Entry, Strokes};
use crate::keys::Keys;

mod dict;
mod find_action;
mod orthography;
mod output;
mod run_action;
#[cfg(test)]
mod test;
mod word_list;

// Public API:

#[derive(Debug)]
pub enum SpecialAction {
	Quit,
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
	/// This is only used for suffixes, because the suffix combined with the previous text may require replacing the previous text, but they can't be grouped into a single `Strokes` because it will interfere with multi-stroke resolution.
	/// If this field is `true`, the text from the previous entry in the backlog should be re-appended if this entry is deleted.
	replaced_previous: bool,
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
