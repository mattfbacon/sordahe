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

use wayland_client::protocol::wl_keyboard::KeyState;
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::{
	self, ZwpInputMethodKeyboardGrabV2,
};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::{
	self, ZwpInputMethodV2,
};

use crate::dict::Dict;
use crate::steno::{Output as StenoOutput, Steno};

mod dict;
mod keys;
mod steno;

#[derive(Debug)]
struct App {
	input: ZwpInputMethodV2,
	serial: u32,
	should_exit: bool,

	steno: Steno,
}

impl App {
	fn run_output(&mut self, output: StenoOutput) {
		self.input.delete_surrounding_text(output.delete, 0);
		self.input.commit_string(output.append);
		self.input.commit(self.serial);
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
				return;
			}

			match key_state {
				KeyState::Pressed => state.steno.key_pressed(key),
				KeyState::Released => {
					if let Some(output) = state.steno.key_released(key) {
						state.run_output(output);
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
		if let zwp_input_method_v2::Event::Done = event {
			state.serial += 1;
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

struct PanicIfImeUnavailable;

impl Dispatch<ZwpInputMethodV2, ()> for PanicIfImeUnavailable {
	fn event(
		_state: &mut Self,
		_proxy: &ZwpInputMethodV2,
		event: <ZwpInputMethodV2 as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		if let zwp_input_method_v2::Event::Unavailable = event {
			panic!("an IME is already registered")
		}
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

	let input = {
		let mut queue = conn.new_event_queue::<PanicIfImeUnavailable>();
		let handle = queue.handle();

		let input = manager.get_input_method(&seat, &handle, ());

		queue.roundtrip(&mut PanicIfImeUnavailable).unwrap();

		input
	};

	let mut queue = conn.new_event_queue::<App>();
	let handle = queue.handle();

	let grab = input.grab_keyboard(&handle, ());

	let mut app = App {
		input,
		serial: 0,
		should_exit: false,
		steno: Steno::new(dict),
	};

	queue.roundtrip(&mut app).unwrap();

	while !app.should_exit {
		queue.blocking_dispatch(&mut app).unwrap();
	}

	grab.release();
	queue.roundtrip(&mut app).unwrap();
}
