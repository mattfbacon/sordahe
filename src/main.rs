use std::collections::VecDeque;

use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::{
	self, ZwpInputMethodKeyboardGrabV2,
};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::{
	self, ZwpInputMethodV2,
};

use crate::dict::{Dict, EntryPart};
use crate::keys::Keys;

mod dict;
mod keys;

const BACKLOG_DEPTH: usize = 16;

#[derive(Debug)]
struct App {
	manager: Option<ZwpInputMethodManagerV2>,
	seat: Option<WlSeat>,
	input: Option<ZwpInputMethodV2>,
	serial: u32,
	keys: Keys,
	should_exit: bool,
	dict: Dict,
	caps: Option<bool>,
	space: bool,
	backlog: VecDeque<Keys>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for App {
	fn event(
		state: &mut Self,
		registry: &wl_registry::WlRegistry,
		event: wl_registry::Event,
		_: &(),
		_: &Connection,
		handle: &QueueHandle<App>,
	) {
		if let wl_registry::Event::Global {
			name, interface, ..
		} = event
		{
			match interface.as_str() {
				"zwp_input_method_manager_v2" => {
					let manager = registry.bind::<ZwpInputMethodManagerV2, _, _>(name, 1, handle, ());
					state.manager = Some(manager);
				}
				"wl_seat" => {
					let seat = registry.bind::<WlSeat, _, _>(name, 8, handle, ());
					state.seat = Some(seat);
				}
				_ => {}
			}
		}
	}
}

impl Dispatch<WlSeat, ()> for App {
	fn event(
		_state: &mut Self,
		_proxy: &WlSeat,
		event: <WlSeat as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		match event {
			wl_seat::Event::Name { .. } | wl_seat::Event::Capabilities { .. } => {}
			_ => {
				dbg!(event);
			}
		}
	}
}

impl Dispatch<ZwpInputMethodManagerV2, ()> for App {
	fn event(
		_state: &mut Self,
		_proxy: &ZwpInputMethodManagerV2,
		event: <ZwpInputMethodManagerV2 as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		dbg!(event);
	}
}

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
			// Escape.
			if key == 1 {
				state.should_exit = true;
			}

			match key_state {
				KeyState::Pressed => {
					if let Some(bit) = Keys::from_code(key) {
						state.keys |= bit;
					}
				}
				KeyState::Released => {
					if !state.keys.is_empty() {
						/*
						state
							.input
							.as_ref()
							.unwrap()
							.commit_string(format!("{:?}\n", state.keys));
						state.input.as_ref().unwrap().commit(state.serial);
						*/
						/*
						state
							.input
							.as_ref()
							.unwrap()
							.set_preedit_string("hello".into(), -1, -1);
						state.input.as_ref().unwrap().commit(state.serial);
						*/
						let input = state.input.as_ref().unwrap();

						eprintln!("{:?} {:?}", state.keys, state.dict.get(&[state.keys]));
						for part in state.dict.get(&[state.keys]).into_iter().flatten() {
							match part {
								EntryPart::Verbatim(text) => {
									let mut text = if state.space {
										[" ", text].concat()
									} else {
										text.clone().into()
									};
									if let Some(caps) = state.caps {
										if let Some((first_pos, first)) = text.char_indices().find(|(_, ch)| *ch != ' ')
										{
											let first = &mut text[first_pos..][..first.len_utf8()];
											if caps {
												first.make_ascii_uppercase();
											} else {
												first.make_ascii_lowercase();
											}
										}
									}
									input.commit_string(text);
									state.caps = None;
									state.space = true;
								}
								EntryPart::SpecialPunct(punct) => {
									if punct.is_sentence_end() {
										input.commit_string(punct.as_str().into());
										state.caps = Some(true);
										state.space = true;
									} else {
									}
								}
								EntryPart::SetCaps(set) => {
									state.caps = Some(*set);
								}
								EntryPart::SetSpace(set) => {
									state.space = *set;
								}
								EntryPart::Glue => todo!(),
								EntryPart::PloverCommand(command) => match *command {},
							}
						}

						state.input.as_ref().unwrap().commit(state.serial);

						state.keys = Keys::empty();
					}
				}
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

fn main() {
	let dict = Dict::load();

	let mut app = App {
		manager: None,
		seat: None,
		input: None,
		serial: 0,
		keys: Keys::empty(),
		should_exit: false,
		dict,
		caps: Some(true),
		space: false,
		backlog: VecDeque::new(),
	};

	let conn = Connection::connect_to_env().unwrap();
	let display = conn.display();
	let mut queue = conn.new_event_queue::<App>();
	let handle = queue.handle();

	let _registry = display.get_registry(&handle, ());

	queue.roundtrip(&mut app).unwrap();

	let input_method =
		app
			.manager
			.as_ref()
			.unwrap()
			.get_input_method(app.seat.as_ref().unwrap(), &handle, ());
	app.input = Some(input_method.clone());

	queue.roundtrip(&mut app).unwrap();

	let grab = input_method.grab_keyboard(&handle, ());

	queue.roundtrip(&mut app).unwrap();

	while !app.should_exit {
		queue.blocking_dispatch(&mut app).unwrap();
	}

	grab.release();
	queue.roundtrip(&mut app).unwrap();
}
