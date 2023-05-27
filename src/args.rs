#![allow(clippy::module_name_repetitions)]

use std::path::PathBuf;
use std::str::FromStr;

use argh::FromArgs;
use thiserror::Error;

/// Stenotype for Wayland.
#[derive(FromArgs, Debug)]
pub struct Args {
	/// path to the dictionary JSON
	#[argh(option, short = 'D', default = r#""dict.json".into()"#)]
	pub dict: PathBuf,
	/// path to the word list
	#[argh(option, short = 'W', default = r#""words.txt".into()"#)]
	pub word_list: PathBuf,
	#[argh(subcommand)]
	pub frontend: Frontend,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum Frontend {
	InputMethod(InputMethodArgs),
	VirtualKeyboard(VirtualKeyboardArgs),
}

/// Run as an input method, translating from the normal keyboard to stenotype.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "input-method")]
pub struct InputMethodArgs {}

#[derive(Debug, Default)]
pub enum StenoProtocol {
	#[default]
	Gemini,
}

#[derive(Debug, Error)]
#[error("unrecognized steno protocol; supported are: gemini")]
pub struct StenoProtocolFromStrError;

impl FromStr for StenoProtocol {
	type Err = StenoProtocolFromStrError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"gemini" => Self::Gemini,
			_ => return Err(StenoProtocolFromStrError),
		})
	}
}

/// Run as an virtual keyboard, taking input from a dedicated stenotype machine.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "virtual-keyboard")]
pub struct VirtualKeyboardArgs {
	/// path to the steno device in `/dev`
	#[argh(option, short = 'd')]
	pub device: Option<String>,
	/// protocol used by the steno device
	#[argh(option, short = 'p', default = "<_>::default()")]
	pub protocol: StenoProtocol,
}

pub fn load() -> Args {
	argh::from_env()
}
