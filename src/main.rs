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

#[derive(Debug, Clone, Copy)]
struct InputState {
	caps: Option<bool>,
	space: bool,
}

#[derive(Debug)]
struct InputEvent {
	strokes: Vec<Keys>,
	len: usize,
	state_before: InputState,
}

#[derive(Debug)]
struct App {
	manager: Option<ZwpInputMethodManagerV2>,
	seat: Option<WlSeat>,
	input: Option<ZwpInputMethodV2>,
	serial: u32,
	keys: Keys,
	should_exit: bool,
	dict: Dict,
	state: InputState,
	backlog: VecDeque<InputEvent>,
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
						let input = state.input.as_ref().unwrap();

						let action = (0..state.backlog.len())
							.find_map(|skip| {
								let events = state.backlog.range(skip..);
								let strokes = events
									.clone()
									.flat_map(|event| &event.strokes)
									.copied()
									.chain(std::iter::once(state.keys))
									.collect::<Vec<_>>();
								eprintln!("trying {strokes:?}");
								state.dict.get(&strokes).map(|entry| {
									(
										entry,
										strokes,
										events.len(),
										events.map(|event| event.len).sum(),
										Some(state.backlog[skip].state_before),
									)
								})
							})
							.or_else(|| {
								state
									.dict
									.get(&[state.keys])
									.map(|entry| (entry, vec![state.keys], 0, 0, None))
							});
						eprintln!("{:?} {action:?}", state.keys);
						if let Some((entry, strokes, pop_backlog, to_delete, restore_state)) = action {
							state.backlog.truncate(state.backlog.len() - pop_backlog);

							if let Some(restore_state) = restore_state {
								state.state = restore_state;
							}

							if to_delete > 0 {
								input.delete_surrounding_text(to_delete.try_into().unwrap(), 0);
							}

							let state_before = state.state;
							let mut send = Some(|text: String| {
								state
									.backlog
									.drain(..state.backlog.len().saturating_sub(BACKLOG_DEPTH - 1));
								state.backlog.push_back(InputEvent {
									strokes,
									len: text.len(),
									state_before,
								});
								input.commit_string(text);
							});

							for part in &entry.0 {
								match part {
									EntryPart::Verbatim(text) => {
										let mut text = if state.state.space {
											[" ", text].concat()
										} else {
											text.clone().into()
										};
										if let Some(caps) = state.state.caps {
											if let Some((first_pos, first)) =
												text.char_indices().find(|(_, ch)| *ch != ' ')
											{
												let first = &mut text[first_pos..][..first.len_utf8()];
												if caps {
													first.make_ascii_uppercase();
												} else {
													first.make_ascii_lowercase();
												}
											}
										}
										send.take().unwrap()(text);
										state.state.caps = None;
										state.state.space = true;
									}
									EntryPart::SpecialPunct(punct) => {
										send.take().unwrap()(punct.as_str().into());
										state.state.space = true;
										state.state.caps = Some(punct.is_sentence_end());
									}
									EntryPart::SetCaps(set) => {
										state.state.caps = Some(*set);
									}
									EntryPart::SetSpace(set) => {
										state.state.space = *set;
									}
									EntryPart::Glue => todo!(),
									EntryPart::PloverCommand(command) => match *command {},
								}
							}

							input.commit(state.serial);
						}

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
		state: InputState {
			caps: Some(true),
			space: false,
		},
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
