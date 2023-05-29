use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::time::Duration;

use anyhow::{anyhow, Context as _};
use memfd::MemfdOptions;
use serialport::{SerialPortType, TTYPort as TtyPort};
use wayland_client::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{delegate_noop, Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use crate::args::{StenoProtocol, VirtualKeyboardArgs};
use crate::bounded_queue::BoundedQueue;
use crate::keys::{Key, Keys};
use crate::steno::{Output, SpecialAction, Steno};

struct NeededProxies {
	manager: Option<ZwpVirtualKeyboardManagerV1>,
	seat: Option<WlSeat>,
}

const ZWP_VIRTUAL_KEYBOARD_MANAGER_V1_VERSION: u32 = 1;
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
				"zwp_virtual_keyboard_manager_v1" => {
					let manager = registry.bind(name, ZWP_VIRTUAL_KEYBOARD_MANAGER_V1_VERSION, handle, ());
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
delegate_noop!(NeededProxies: ignore ZwpVirtualKeyboardManagerV1);

struct App;

delegate_noop!(App: ZwpVirtualKeyboardV1);

fn discover_device() -> anyhow::Result<String> {
	let ports = serialport::available_ports().context("enumerating serial ports")?;
	let (path, _) = ports
		.into_iter()
		.filter_map(|port| {
			let SerialPortType::UsbPort(ty) = port.port_type else { return None; };
			Some((port.port_name, ty))
		})
		.find(|(_name, ty)| ty.manufacturer.as_deref() == Some("Noll_Electronics_LLC"))
		.ok_or_else(|| anyhow!("could not find Nolltronics device in available ports"))?;
	Ok(path)
}

#[derive(Debug)]
struct GeminiDevice<I> {
	inner: I,
}

const BAUD: u32 = 9600;

impl GeminiDevice<TtyPort> {
	fn open(path: &str) -> anyhow::Result<Self> {
		let inner = serialport::new(path, BAUD)
			.timeout(Duration::from_secs(u32::MAX.into()))
			.open_native()?;

		Ok(Self { inner })
	}
}

const GEMINI_LUT: [Option<Key>; 64] = [
	Some(Key::Z),
	None,
	None,
	Some(Key::NumberBar),
	None,
	Some(Key::NumberBar),
	None,
	None,
	Some(Key::D),
	Some(Key::S2),
	Some(Key::T2),
	Some(Key::G),
	Some(Key::L),
	Some(Key::B),
	Some(Key::P2),
	None,
	Some(Key::R2),
	Some(Key::F),
	Some(Key::U),
	Some(Key::E),
	Some(Key::Star),
	Some(Key::Star),
	None,
	None,
	None,
	None,
	Some(Key::Star),
	Some(Key::Star),
	Some(Key::O),
	Some(Key::A),
	Some(Key::R),
	None,
	Some(Key::H),
	Some(Key::W),
	Some(Key::P),
	Some(Key::K),
	Some(Key::T),
	Some(Key::S),
	Some(Key::S),
	None,
	None,
	None,
	Some(Key::NumberBar),
	None,
	Some(Key::NumberBar),
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
	None,
];

impl<I: Read> Iterator for GeminiDevice<I> {
	type Item = anyhow::Result<Keys>;

	fn next(&mut self) -> Option<anyhow::Result<Keys>> {
		let mut buf = [0u8; 8];
		let res = self.inner.read_exact(&mut buf[2..]);
		match res {
			Err(error) if error.kind() == ErrorKind::UnexpectedEof => return None,
			Err(error) => return Some(Err(error).context("IO error reading from device")),
			Ok(..) => {}
		}

		assert!(buf[2] & 0x80 > 0);
		buf[2] &= !0x80;

		let raw = u64::from_be_bytes(buf);
		let keys = (0..u64::BITS)
			.filter(|bit| raw & (1 << bit) > 0)
			.filter_map(|bit| GEMINI_LUT[bit as usize])
			.collect();
		Some(Ok(keys))
	}
}

const KEYMAP: &str = include_str!("../../keymap.xkb");

const MOD_NONE: u32 = 0;
const MOD_SHIFT: u32 = 1 << 0;
const MOD_CONTROL: u32 = 1 << 2;
const GROUP: u32 = 0;

const KEYCODE_BASE: u32 = 8;
const BACKSPACE: u8 = 8;

struct Keyboard {
	inner: ZwpVirtualKeyboardV1,
	serial: u32,
}

impl Keyboard {
	fn new(inner: ZwpVirtualKeyboardV1) -> anyhow::Result<Self> {
		let keymap_file = MemfdOptions::new()
			.allow_sealing(true)
			.close_on_exec(true)
			.create("sordahe-keymap")
			.context("creating keymap memfd")?;
		keymap_file
			.as_file()
			.write_all(KEYMAP.as_bytes())
			.context("writing to memfd")?;

		inner.keymap(
			KeymapFormat::XkbV1 as u32,
			keymap_file.as_raw_fd(),
			KEYMAP.len().try_into().unwrap(),
		);

		Ok(Self { inner, serial: 0 })
	}

	fn next_serial(&mut self) -> u32 {
		let ret = self.serial;
		self.serial += 1;
		ret
	}

	fn key_raw(&mut self, key: u32, pressed: bool) {
		let state = if pressed {
			KeyState::Pressed
		} else {
			KeyState::Released
		} as u32;
		let serial = self.next_serial();
		self.inner.key(serial, key, state);
	}

	fn key(&mut self, key: u32) {
		self.key_raw(key, true);
		self.key_raw(key, false);
	}

	fn set_modifiers(&self, ctrl: bool, shift: bool) {
		let mut modifiers = 0;
		if ctrl {
			modifiers |= MOD_CONTROL;
		}
		if shift {
			modifiers |= MOD_SHIFT;
		}
		self.inner.modifiers(modifiers, MOD_NONE, MOD_NONE, GROUP);
	}

	fn reset_modifiers(&self) {
		self.set_modifiers(false, false);
	}

	fn has_ascii(byte: u8) -> bool {
		(8..=126).contains(&byte)
	}

	fn type_ascii(&mut self, ascii: u8) {
		debug_assert!(Self::has_ascii(ascii), "out of viable ASCII range");
		let key = u32::from(ascii) - KEYCODE_BASE;
		self.key(key);
	}

	fn type_unicode(&mut self, ch: char) {
		self.set_modifiers(true, true);
		self.type_ascii(b'u');
		self.reset_modifiers();
		self.type_ascii(b'0');
		self.type_ascii(b'x');
		let mut buf = [b'\0'; 8];
		write!(&mut buf.as_mut_slice(), "{:x}", u32::from(ch)).unwrap();
		for ch in buf.into_iter().take_while(|&b| b != b'\0') {
			self.type_ascii(ch);
		}
		self.type_ascii(b'\n');
	}

	fn backspace(&mut self) {
		self.type_ascii(BACKSPACE);
	}

	fn type_str(&mut self, s: &str) {
		for ch in s.chars() {
			if let Some(byte) = u8::try_from(ch).ok().filter(|&b| Self::has_ascii(b)) {
				self.type_ascii(byte);
			} else {
				self.type_unicode(ch);
			}
		}
	}
}

pub fn run(mut steno: Steno, args: VirtualKeyboardArgs) -> anyhow::Result<()> {
	let device_path = args.device.map_or_else(discover_device, Ok)?;
	let StenoProtocol::Gemini = args.protocol;
	let device = GeminiDevice::open(&device_path).with_context(|| {
		format!(
			"opening device at {device_path:?} with protocol {:?}",
			args.protocol,
		)
	})?;

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
			.ok_or_else(|| anyhow!("no zwp_virtual_keyboard_manager_v1 found in registry"))?;
		let seat = needed
			.seat
			.ok_or_else(|| anyhow!("no wl_seat found in registry"))?;
		(manager, seat)
	};

	let mut queue = conn.new_event_queue::<App>();
	let handle = queue.handle();

	let keyboard = manager.create_virtual_keyboard(&seat, &handle, ());
	let mut keyboard = Keyboard::new(keyboard).context("creating virtual keyboard")?;

	queue.roundtrip(&mut App)?;

	let mut buffer = BoundedQueue::new(100);

	for keys in device {
		let keys = keys.context("reading keys from device")?;
		eprintln!("{keys:#}");
		let output = steno.run_keys(keys).map(|()| steno.flush());

		match output {
			Ok(mut output) => {
				output.use_buffer(&mut buffer);

				let Output {
					delete_words,
					delete,
					append,
				} = output;

				for _ in 0..delete.chars() {
					keyboard.backspace();
				}

				keyboard.set_modifiers(true, false);
				for _ in 0..delete_words {
					keyboard.backspace();
				}
				keyboard.reset_modifiers();

				keyboard.type_str(&append);

				queue.roundtrip(&mut App)?;
			}
			Err(SpecialAction::Quit) => break,
		}
	}

	Ok(())
}
