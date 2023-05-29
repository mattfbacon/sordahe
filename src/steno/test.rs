use once_cell::sync::Lazy;

use crate::dict::{Dict, Strokes};
use crate::keys::Keys;
use crate::steno::Steno;
use crate::word_list::WordList;

fn steno_to_string(dict: &Dict, word_list: &WordList, input: &[Keys]) -> String {
	let mut steno = Steno::new(dict, word_list);

	for &keys in input {
		steno.run_keys(keys).unwrap();
	}

	let output = steno.flush();
	assert_eq!(output.delete.bytes(), 0);
	assert_eq!(output.delete_words, 0);
	output.append
}

static DICT: Lazy<Dict> =
	Lazy::new(|| serde_json::from_str(include_str!("../../dict.json")).unwrap());

static WORD_LIST: Lazy<WordList> = Lazy::new(|| include_str!("../../words.txt").parse().unwrap());

const TESTS: &[(&str, &str)] = &[
	// Basic
	("TH/S/AEU/TEFT", "This is a test"),
	// Punctuation
	("TEFT/-P/TEFT/-P/TEFT", "Test. Test. Test"),
	// Chords
	("HEL/HRO", "Hello"),
	("HEL/HRO/*", "Hell"),
	("HEL/HRO/HRA", "Hello la"),
	// Numbers
	("123", "123"),
	("123EU", "321"),
	("1D", "11"),
	("123D", "123123"),
	("123Z", "12300"),
	("123DZ", "$12300"),
	("50", "50"),
	("056", "506"),
	("12K", "12:00"),
	("12BG", "12:00"),
	("1234567890EUBGDZ", "$987605432100:00"),
	// Orthography
	// https://sites.google.com/site/learnplover/lesson-7-orthography-rules-and-suffix-keys
	("TEFT/G", "Testing"),
	("TEFT/-S", "Tests"),
	("TEFTS", "Tests"),
	("AR/TEUS/TEUBG", "Artistic"),
	("AR/TEUS/TEUBG/HREU", "Artistically"),
	("HROPBLG/EUBG", "Logic"),
	("HROPBLG/EUBG/HREU", "Logically"),
	("STAUT", "Statute"),
	("STAUT/REU", "Statutory"),
	("APL/HRAEUT", "Ambulate"),
	("APL/HRAEUT/REU", "Ambulatory"),
	("TPREBG", "Frequent"),
	("TPREBG/SEU", "Frequency"),
	("RE/SKWREPBT", "Regent"),
	("RE/SKWREPBT/SEU", "Regency"),
	("AD/KWAT", "Adequate"),
	("AD/KWAT/SEU", "Adequacy"),
	("STAEB", "Establish"),
	("STAEB/-S", "Establishes"),
	("STAEBS", "Establishes"),
	("SPAOEFP", "Speech"),
	("SPAOEFP/-S", "Speeches"),
	("SPAOEFPS", "Speeches"),
	("KHER/REU", "Cherry"),
	("KHER/REU/-S", "Cherries"),
	// ("KHER/REUS", "Cherries"), // REUS has a legitimate conflicting entry for "{^aries}".
	("TKAOEU", "Die"),
	("TKAOEU/G", "Dying"),
	("TKAOEUG", "Dying"),
	("PHET/HRURPBLG", "Metallurgy"),
	("PHET/HRURPBLG/EUFT", "Metallurgist"),
	("PWAOUT", "Beauty"),
	("PWAOUT/FL", "Beautiful"),
	("WREU", "Write"),
	("WREU/*EPB", "Written"),
	("TPRAOE", "Free"),
	("TPRAOE/D", "Freed"),
	("TPRAOED", "Freed"),
	("TPHAR/RAEUT", "Narrate"),
	("TPHAR/RAEUT/G", "Narrating"),
	("TPHAR/RAEUTG", "Narrating"),
	("TKEFR", "Defer"),
	("TKEFR/D", "Deferred"),
	("TKEFRD", "Deferred"),
	("TEUPBT", "Tint"),
	("TEUPBT/G", "Tinting"),
	("TEUPBTG", "Tinting"),
	("SEUT", "Sit"),
	("SEUT/G", "Sitting"),
	// ("SEUTG", "Sitting"), // Has a legitimate conflicting entry for "signature".
	("RUB", "Rub"),
	("RUB/*ER", "Rubber"),
	("PWEUG", "Big"),
	("PWEUG/EFT", "Biggest"),
	("SELT", "Settle"),
	("SELT/D", "Settled"),
	("SELTD", "Settled"),
	("SELT/D/TEFT", "Settled test"),
	("SELT/D/*", "Settle"),
	("TH/S/AEU/TEFT/SELT/D/*/*/*", "This is a"),
	("PHAOET/G/PHAOET/G", "Meeting meeting"),
	// Glue
	("KP*", "x"),
	("KP*/KP*/KP*", "xxx"),
	("TP*DZ/*U/*EU/HR*", "Fuil"),
];

#[test]
fn test() {
	let mut success = true;

	for &(raw_input, expected_output) in TESTS {
		let input_strokes = raw_input.parse::<Strokes>().unwrap().0;
		let actual_output = steno_to_string(&DICT, &WORD_LIST, &input_strokes);
		let correct = actual_output == expected_output;
		success &= correct;
		if !correct {
			println!("failed: input {raw_input:?}, expected output {expected_output:?}, actual output {actual_output:?}");
		}
	}

	assert!(success, "some tests failed");
}
