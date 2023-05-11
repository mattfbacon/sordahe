use std::collections::VecDeque;

use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::{
	self, ZwpInputMethodKeyboardGrabV2,
};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::{
	self, ZwpInputMethodV2,
};

use crate::dict::{Dict, Entry, EntryPart, Strokes};
use crate::keys::Keys;

mod dict;
mod keys;

const BACKLOG_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy)]
struct InputState {
	caps: Option<bool>,
	space: bool,
}

#[derive(Debug)]
struct InputEvent {
	strokes: Strokes,
	len: usize,
	state_before: InputState,
}

#[derive(Debug)]
struct App {
	dict: Dict,
	input: ZwpInputMethodV2,
	serial: u32,
	keys: Keys,
	should_exit: bool,
	state: InputState,
	backlog: VecDeque<InputEvent>,
}

struct Action {
	entry: Entry,
	strokes: Strokes,
	pop_backlog: usize,
	to_delete: usize,
	restore_state: Option<InputState>,
}

impl App {
	fn key_pressed(&mut self, code: u32) {
		if let Some(bit) = Keys::from_code(code) {
			self.keys |= bit;
		}
	}

	fn key_released(&mut self, _code: u32) {
		let keys = std::mem::take(&mut self.keys);

		if keys.is_empty() {
			return;
		}

		let action = self.find_action(keys);

		let Some(Action {
			entry,
			strokes,
			pop_backlog,
			to_delete,
			restore_state,
		}) = action else { return; };

		self.backlog.truncate(self.backlog.len() - pop_backlog);

		if let Some(restore_state) = restore_state {
			self.state = restore_state;
		}

		self.delete(to_delete);

		let state_before = self.state;
		let mut send_ = Some(|text: String| {
			self
				.backlog
				.drain(..self.backlog.len().saturating_sub(BACKLOG_DEPTH - 1));
			self.backlog.push_back(InputEvent {
				strokes,
				len: text.len(),
				state_before,
			});
			self.input.commit_string(text);
		});
		let mut send = |text| send_.take().expect("you gotta actually implement it now")(text);

		for part in &*entry.0 {
			match part {
				EntryPart::Verbatim(text) => {
					let mut text = if self.state.space {
						[" ", text].concat()
					} else {
						text.clone().into()
					};
					if let Some(caps) = self.state.caps {
						if let Some((first_pos, first)) = text.char_indices().find(|(_, ch)| *ch != ' ') {
							let first = &mut text[first_pos..][..first.len_utf8()];
							if caps {
								first.make_ascii_uppercase();
							} else {
								first.make_ascii_lowercase();
							}
						}
					}
					send(text);
					self.state.caps = None;
					self.state.space = true;
				}
				EntryPart::SpecialPunct(punct) => {
					send(punct.as_str().into());
					self.state.space = true;
					self.state.caps = Some(punct.is_sentence_end());
				}
				EntryPart::SetCaps(set) => {
					self.state.caps = Some(*set);
				}
				EntryPart::SetSpace(set) => {
					self.state.space = *set;
				}
				EntryPart::Glue => todo!(),
				EntryPart::PloverCommand(command) => match *command {},
			}
		}

		self.input.commit(self.serial);
	}

	fn find_action(&self, this_keys: Keys) -> Option<Action> {
		(0..=self.backlog.len()).find_map(|skip| {
			let events = self.backlog.range(skip..);
			let strokes = events
				.clone()
				.flat_map(|event| &event.strokes.0)
				.copied()
				.chain(std::iter::once(this_keys))
				.collect::<Vec<_>>();
			self.dict.get(&strokes).map(|entry| Action {
				entry: entry.clone(),
				strokes: Strokes(strokes),
				pop_backlog: events.len(),
				to_delete: events.map(|event| event.len).sum::<usize>(),
				restore_state: self.backlog.get(skip).map(|event| event.state_before),
			})
		})
	}

	fn delete(&self, amount: usize) {
		if amount > 0 {
			self
				.input
				.delete_surrounding_text(amount.try_into().unwrap(), 0);
		}
	}
}

const ESCAPE_KEY: u32 = 1;

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for App {
	fn event(
		state: &mut Self,
		_proxy: &ZwpInputMethodKeyboardGrabV2,
		event: <ZwpInputMethodKeyboardGrabV2 as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		if let zwp_input_method_keyboard_grab_v2::Event::Key {
			key,
			state: WEnum::Value(key_state),
			..
		} = event
		{
			if key == ESCAPE_KEY {
				state.should_exit = true;
			}

			match key_state {
				KeyState::Pressed => state.key_pressed(key),
				KeyState::Released => state.key_released(key),
				_ => {}
			}
		}
	}
}

impl Dispatch<ZwpInputMethodV2, ()> for App {
	fn event(
		state: &mut Self,
		_proxy: &ZwpInputMethodV2,
		event: <ZwpInputMethodV2 as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		match event {
			zwp_input_method_v2::Event::Unavailable => panic!("an IME is already registered"),
			zwp_input_method_v2::Event::Done => {
				state.serial += 1;
			}
			_ => {}
		}
	}
}

struct NeededProxies {
	manager: Option<ZwpInputMethodManagerV2>,
	seat: Option<WlSeat>,
}

const ZWP_INPUT_METHOD_MANAGER_V2_VERSION: u32 = 1;
const WL_SEAT_VERSION: u32 = 8;

impl Dispatch<wl_registry::WlRegistry, ()> for NeededProxies {
	fn event(
		state: &mut Self,
		registry: &wl_registry::WlRegistry,
		event: wl_registry::Event,
		_: &(),
		_: &Connection,
		handle: &QueueHandle<Self>,
	) {
		if let wl_registry::Event::Global {
			name, interface, ..
		} = event
		{
			match interface.as_str() {
				"zwp_input_method_manager_v2" => {
					let manager = registry.bind(name, ZWP_INPUT_METHOD_MANAGER_V2_VERSION, handle, ());
					state.manager = Some(manager);
				}
				"wl_seat" => {
					let seat = registry.bind(name, WL_SEAT_VERSION, handle, ());
					state.seat = Some(seat);
				}
				_ => {}
			}
		}
	}
}

delegate_noop!(NeededProxies: ignore WlSeat);
delegate_noop!(NeededProxies: ignore ZwpInputMethodManagerV2);

struct Ignored;

impl<T: Proxy, U> Dispatch<T, U> for Ignored {
	fn event(
		_state: &mut Self,
		_proxy: &T,
		_event: <T as Proxy>::Event,
		_data: &U,
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
	}
}

fn main() {
	let dict = Dict::load();

	let conn = Connection::connect_to_env().unwrap();
	let display = conn.display();

	let (manager, seat) = {
		let mut needed = NeededProxies {
			manager: None,
			seat: None,
		};

		let mut queue = conn.new_event_queue::<NeededProxies>();
		let handle = queue.handle();

		display.get_registry(&handle, ());

		queue.roundtrip(&mut needed).unwrap();

		(needed.manager.unwrap(), needed.seat.unwrap())
	};

	let input = manager.get_input_method(&seat, &conn.new_event_queue::<Ignored>().handle(), ());

	let mut queue = conn.new_event_queue::<App>();
	let handle = queue.handle();

	let grab = input.grab_keyboard(&handle, ());

	let mut app = App {
		dict,
		input,
		serial: 0,
		keys: Keys::empty(),
		should_exit: false,
		state: InputState {
			caps: Some(true),
			space: false,
		},
		backlog: VecDeque::new(),
	};

	queue.roundtrip(&mut app).unwrap();

	while !app.should_exit {
		queue.blocking_dispatch(&mut app).unwrap();
	}

	grab.release();
	queue.roundtrip(&mut app).unwrap();
}
