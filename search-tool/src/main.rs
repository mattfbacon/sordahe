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

use std::collections::HashMap;

fn main() {
	let dict: HashMap<Box<str>, Box<str>> =
		serde_json::from_str(&std::fs::read_to_string("../dict.json").unwrap()).unwrap();

	eprint!("> ");
	for search in std::io::stdin().lines() {
		let search = search.unwrap();
		let mut entries = dict
			.iter()
			.filter(|(_stroke, text)| ***text == search)
			.map(|(stroke, _text)| stroke)
			.collect::<Vec<_>>();
		entries.sort_by_key(|entry| (entry.chars().filter(|&ch| ch == '/').count(), entry.len()));

		for entry in entries {
			println!("{entry}");
		}

		eprint!("> ");
	}
}
