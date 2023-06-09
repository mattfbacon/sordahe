use anyhow::{anyhow, ensure, Context as _};
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

use crate::args::InputMethodArgs;
use crate::bounded_queue::BoundedQueue;
use crate::keys::{Key, Keys};
use crate::steno::{Output, SpecialAction, Steno};

#[derive(Debug)]
pub struct App {
	input: ZwpInputMethodV2,
	serial: u32,
	should_exit: bool,
	keys_seen: Keys,
	keys_current: Keys,

	steno: Steno,
	buffer: BoundedQueue<u8>,
}

impl App {
	fn key_pressed(&mut self, key: Key) {
		self.keys_seen |= key;
		self.keys_current |= key;
	}

	fn key_released(&mut self, key: Key) {
		self.keys_current &= !key;
		if self.keys_current.is_empty() && !self.keys_seen.is_empty() {
			let keys = std::mem::take(&mut self.keys_seen);
			eprintln!("{keys:#}");
			let output = self.steno.run_keys(keys).map(|()| self.steno.flush());
			self.run_output(output);
		}
	}

	fn run_output(&mut self, output: Result<Output, SpecialAction>) {
		match output {
			Ok(mut output) => {
				output.use_buffer(&mut self.buffer);

				let Output {
					delete_words,
					delete,
					append,
				} = output;

				// We want to delete words, but this isn't really possible as an input method, so we'll delete a single character instead.
				let delete = (delete_words + delete.bytes())
					.try_into()
					.expect("deletion overflowed u32");
				self.input.delete_surrounding_text(delete, 0);
				self.input.commit_string(append);
				self.input.commit(self.serial);
			}
			Err(SpecialAction::Quit) => {
				self.should_exit = true;
			}
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
				return;
			}

			let Some(key) = Key::from_code(key) else { return; };

			match key_state {
				KeyState::Pressed => {
					state.key_pressed(key);
				}
				KeyState::Released => {
					state.key_released(key);
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

struct CheckIfImeAvailable {
	available: bool,
}

impl Dispatch<ZwpInputMethodV2, ()> for CheckIfImeAvailable {
	fn event(
		state: &mut Self,
		_proxy: &ZwpInputMethodV2,
		event: <ZwpInputMethodV2 as wayland_client::Proxy>::Event,
		_data: &(),
		_conn: &Connection,
		_qhandle: &QueueHandle<Self>,
	) {
		if let zwp_input_method_v2::Event::Unavailable = event {
			state.available = false;
		}
	}
}

pub fn run(steno: Steno, InputMethodArgs {}: InputMethodArgs) -> anyhow::Result<()> {
	let conn = Connection::connect_to_env().context("connecting to Wayland server")?;
	let display = conn.display();

	let (manager, seat) = {
		let mut needed = NeededProxies {
			manager: None,
			seat: None,
		};

		let mut queue = conn.new_event_queue::<NeededProxies>();
		let handle = queue.handle();

		display.get_registry(&handle, ());

		queue.roundtrip(&mut needed)?;

		let manager = needed
			.manager
			.ok_or_else(|| anyhow!("no zwp_input_method_manager_v2 found in registry"))?;
		let seat = needed
			.seat
			.ok_or_else(|| anyhow!("no wl_seat found in registry"))?;
		(manager, seat)
	};

	let input = {
		let mut queue = conn.new_event_queue::<CheckIfImeAvailable>();
		let handle = queue.handle();

		let input = manager.get_input_method(&seat, &handle, ());

		let mut check = CheckIfImeAvailable { available: true };
		queue.roundtrip(&mut check)?;
		ensure!(check.available, "an IME is already registered");

		input
	};

	let mut queue = conn.new_event_queue::<App>();
	let handle = queue.handle();

	let grab = input.grab_keyboard(&handle, ());

	let mut app = App {
		input,
		serial: 0,
		should_exit: false,
		keys_current: Keys::empty(),
		keys_seen: Keys::empty(),

		steno,
		buffer: BoundedQueue::new(100),
	};

	queue.roundtrip(&mut app)?;

	while !app.should_exit {
		queue.blocking_dispatch(&mut app)?;
	}

	grab.release();
	queue.roundtrip(&mut app)?;

	Ok(())
}
