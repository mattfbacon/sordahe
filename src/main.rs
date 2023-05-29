#![deny(
	absolute_paths_not_starting_with_crate,
	keyword_idents,
	macro_use_extern_crate,
	meta_variable_misuse,
	missing_abi,
	missing_copy_implementations,
	non_ascii_idents,
	nonstandard_style,
	noop_method_call,
	pointer_structural_match,
	private_in_public,
	rust_2018_idioms,
	unused_qualifications
)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use anyhow::Context as _;

use crate::args::Frontend;
use crate::dict::Dict;
use crate::steno::Steno;
use crate::word_list::WordList;

mod args;
mod bounded_queue;
mod chars_or_bytes;
mod dict;
mod frontends;
mod keys;
mod steno;
mod word_list;

fn main() -> anyhow::Result<()> {
	let args = args::load();

	let dict =
		Dict::load(&args.dict).with_context(|| format!("loading dictionary from {:?}", args.dict))?;
	let word_list = WordList::load(&args.word_list)
		.with_context(|| format!("loading word list from {:?}", args.word_list))?;
	let steno = Steno::new(dict, word_list);

	match args.frontend {
		Frontend::InputMethod(args) => crate::frontends::input_method::run(steno, args),
		Frontend::VirtualKeyboard(args) => crate::frontends::virtual_keyboard::run(steno, args),
	}
	.context("running frontend")
}
