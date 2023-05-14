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

use crate::args::Frontend;
use crate::dict::Dict;
use crate::steno::Steno;

mod args;
mod dict;
mod frontends;
mod keys;
mod steno;

fn main() {
	let args = args::load();

	let dict = Dict::load(&args.dict);
	let steno = Steno::new(dict);

	match args.frontend {
		Frontend::InputMethod(args) => {
			crate::frontends::input_method::run(steno, args);
		}
		Frontend::VirtualKeyboard(args) => {
			crate::frontends::virtual_keyboard::run(steno, args);
		}
	}
}
