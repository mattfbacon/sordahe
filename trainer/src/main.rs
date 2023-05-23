use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead as _, BufReader};
use std::ops::Range;
use std::path::Path;
use std::time::Instant;

use egui::{Align2, DragValue, Frame, Vec2};
use egui_file::FileDialog;
use rand::Rng;

fn main() {
	let native_options = eframe::NativeOptions::default();
	eframe::run_native("App", native_options, Box::new(|cc| Box::new(App::new(cc)))).unwrap();
}

struct Words {
	name: String,
	words: Vec<String>,
	range: Range<usize>,
	current_index: usize,
	type_buf: String,
}

fn gen_range(range: Range<usize>) -> usize {
	rand::thread_rng().gen_range(range)
}

impl Words {
	fn read(path: &Path) -> io::Result<Self> {
		let name = path
			.file_stem()
			.or_else(|| path.file_name())
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| "(unknown)".into());
		let words = BufReader::new(File::open(path)?)
			.lines()
			.collect::<Result<Vec<_>, _>>()?;
		if words.is_empty() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "no words"));
		}
		let range = 0..words.len();
		let current_index = gen_range(range.clone());
		Ok(Self {
			name,
			words,
			range,
			current_index,
			type_buf: String::with_capacity(64),
		})
	}

	fn current(&self) -> &str {
		&self.words[self.current_index]
	}

	fn is_correct(&self) -> bool {
		self.current().eq_ignore_ascii_case(self.type_buf.trim())
	}

	fn next(&mut self) {
		self.current_index = {
			let index = gen_range(self.range.start..self.range.end - 1);
			if index >= self.current_index {
				index + 1
			} else {
				index
			}
		};
		self.type_buf.clear();
	}

	fn set_range(&mut self, range: Range<usize>) {
		self.range = range.clone();
		if !range.contains(&self.current_index) {
			self.current_index = gen_range(range);
		}
	}
}

struct RollingAverage {
	size: usize,
	buf: VecDeque<f32>,
	average: f32,
}

impl RollingAverage {
	fn new(size: usize) -> Self {
		Self {
			size,
			buf: VecDeque::with_capacity(size),
			average: f32::NAN,
		}
	}

	fn push(&mut self, value: f32) {
		if self.buf.len() == self.size {
			self.buf.pop_front();
		}
		self.buf.push_back(value);
		self.average = self.buf.iter().sum::<f32>() / self.buf.len() as f32;
	}
}

struct App {
	words: Option<Result<Words, String>>,
	open_words: FileDialog,
	last_word_time: Option<Instant>,
	wpm: RollingAverage,
	was_correct: bool,
}

impl App {
	fn new(_: &eframe::CreationContext<'_>) -> Self {
		Self {
			words: None,
			open_words: FileDialog::open_file(Some(std::env::current_dir().unwrap()))
				.show_new_folder(false),
			last_word_time: None,
			wpm: RollingAverage::new(10),
			was_correct: false,
		}
	}
}

const SECONDS_PER_MINUTE: f32 = 60.0;

impl eframe::App for App {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		self.open_words.show(ctx);

		if let Some(Err(error)) = &self.words {
			let mut open = true;
			egui::Window::new("Error reading words")
				.collapsible(false)
				.resizable(false)
				.anchor(Align2::CENTER_CENTER, Vec2::ZERO)
				.open(&mut open)
				.show(ctx, |ui| {
					ui.label(error);
				});
			if !open {
				self.words = None;
			}
		}

		if self.open_words.selected() {
			let path = self.open_words.path().unwrap();
			self.words = Some(Words::read(&path).map_err(|error| error.to_string()));
		}

		egui::CentralPanel::default().show(ctx, |ui| {
			ui.heading("Steno Practice");
			ui.horizontal(|ui| {
				if let Some(Ok(words)) = &self.words {
					ui.label(format!("Words: {:?}", words.name));
				} else {
					ui.label("(No words loaded)");
				}
				if ui.button("Load words").clicked() {
					self.open_words.open();
				}
			});
			if let Some(Ok(words)) = &mut self.words {
				ui.horizontal(|ui| {
					ui.label("Range:");
					let mut range = words.range.clone();
					range.start += 1;
					range.end += 1;
					ui.add(DragValue::new(&mut range.start).clamp_range(0..=(range.end - 1)));
					ui.add(
						DragValue::new(&mut range.end).clamp_range((range.start + 1)..=(words.words.len())),
					);
					range.start -= 1;
					range.end -= 1;
					if range != words.range {
						words.set_range(range);
					}
				});
			}

			if let Some(Ok(words)) = &mut self.words {
				Frame::group(ui.style()).show(ui, |ui| {
					let word = words.current();
					ui.heading(word);
					ui.text_edit_singleline(&mut words.type_buf);
					let is_correct = words.is_correct();
					if is_correct && self.was_correct {
						words.next();
						if let Some(last) = self.last_word_time {
							self.wpm.push(last.elapsed().as_secs_f32().recip())
						}
						self.last_word_time = Some(Instant::now());
						self.was_correct = false;
					} else {
						self.was_correct = is_correct;
						if is_correct {
							ctx.request_repaint_after(std::time::Duration::from_millis(50));
						}
					}
				});
			}

			if let Some(last) = self.wpm.buf.iter().copied().next_back() {
				ui.label(format!("Last WPM: {:.1}", last * SECONDS_PER_MINUTE));
			}
			if !self.wpm.average.is_nan() {
				ui.label(format!(
					"Last 10 WPM: {:.1}",
					self.wpm.average * SECONDS_PER_MINUTE
				));
			}
		});
	}
}
