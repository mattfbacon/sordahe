pub fn apply_orthography_rules(first: &str, second: &str) -> Option<String> {
	const RULES: &[(&str, &str, &str)] = &[
		("ic", "ly", "ically"),
		("te", "ry", "tory"),
		("t", "cy", "cy"),
		("te", "cy", "cy"),
		("sh", "s", "shes"),
		("s", "s", "ses"),
		("each", "s", "eaches"),
		("eech", "s", "eeches"),
		("y", "s", "ies"),
		("y", "ed", "ied"),
		("ie", "ing", "ying"),
		("y", "ist", "ist"),
		("y", "ful", "iful"),
		("te", "en", "tten"),
		("e", "en", "en"),
		("ee", "e", "ee"),
		("e", "ing", "ing"),
		("at", "e", "atte"),
		("ag", "e", "agge"),
		("ab", "e", "abbe"),
		("at", "i", "atti"),
		("ag", "i", "aggi"),
		("ab", "i", "abbi"),
		("an", "i", "anni"),
		("et", "e", "ette"),
		("eg", "e", "egge"),
		("eb", "e", "ebbe"),
		("et", "i", "etti"),
		("eg", "i", "eggi"),
		("eb", "i", "ebbi"),
		("en", "i", "enni"),
		("it", "e", "itte"),
		("ig", "e", "igge"),
		("ib", "e", "ibbe"),
		("it", "i", "itti"),
		("ig", "i", "iggi"),
		("ib", "i", "ibbi"),
		("in", "i", "inni"),
		("ot", "e", "otte"),
		("og", "e", "ogge"),
		("ob", "e", "obbe"),
		("ot", "i", "otti"),
		("og", "i", "oggi"),
		("ob", "i", "obbi"),
		("on", "i", "onni"),
		("ut", "e", "utte"),
		("ug", "e", "ugge"),
		("ub", "e", "ubbe"),
		("ut", "i", "utti"),
		("ug", "i", "uggi"),
		("ub", "i", "ubbi"),
		("un", "i", "unni"),
		("e", "ed", "ed"),
	];

	for (first_suffix, second_prefix, replacement) in RULES {
		if let Some(first) = first.strip_suffix(first_suffix) {
			if let Some(second) = second.strip_prefix(second_prefix) {
				return Some([first, replacement, second].concat());
			}
		}
	}

	None
}
