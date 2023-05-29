use once_cell::sync::Lazy;
use regex::Regex;

pub fn apply_orthography_rules(first: &str, second: &str) -> Option<String> {
	const RULES_RAW: &[(&str, &str, &str)] = &[
		("ic", "ly", "ically"),
		("te", "ry", "tory"),
		("te?", "cy", "cy"),
		("s(h?)", "s", "s${1}es"),
		("e([ae])?ch", "s", "e${1}ches"),
		("y", "s", "ies"),
		("y", "ed", "ied"),
		("ie", "ing", "ying"),
		("y", "ist", "ist"),
		("y", "ful", "iful"),
		("te", "en", "tten"),
		("e", "(en|ed|ing)", "$1"),
		("ee", "e", "ee"),
		("([aeiou])([gbtnr])", "([ei])", "$1$2$2$3"),
	];

	static RULES: Lazy<Vec<(Regex, &str)>> = Lazy::new(|| {
		RULES_RAW
			.iter()
			.copied()
			.map(|(first_suffix, second_prefix, replacement)| {
				(
					Regex::new(&[first_suffix, "\0", second_prefix].concat())
						.expect("internal regex is broken"),
					replacement,
				)
			})
			.collect()
	});

	let concat = [first, "\0", second].concat();
	RULES
		.iter()
		.find(|(regex, _replacement)| regex.is_match(&concat))
		.map(|(regex, replacement)| regex.replace(&concat, *replacement).into_owned())
}
