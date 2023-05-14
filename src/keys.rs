use std::fmt::{self, Display, Formatter, Write as _};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

macro_rules! key_enum {
	($($keys:ident),* $(,)?) => {
		#[derive(Clone, Copy, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
		pub enum Key {
			$($keys,)*
		}

		impl Key {
			const ALL: &[Self] = &[$(Self::$keys,)*];
		}

		impl TryFrom<u32> for Key {
			type Error = ();

			fn try_from(index: u32) -> Result<Self, Self::Error> {
				Self::ALL.get(index as usize).copied().ok_or(())
			}
		}

		paste::paste! {
			impl Keys {
				$(pub const [<$keys:snake:upper>]: Self = Keys::single(Key::$keys);)*
			}
		}
	};
}

key_enum! {
	NumberBar,
	S,
	T,
	K,
	P,
	W,
	H,
	R,
	A,
	O,
	Star,
	E,
	U,
	F,
	R2,
	P2,
	B,
	L,
	G,
	T2,
	S2,
	D,
	Z,
}

impl Key {
	#[allow(clippy::match_same_arms /* sequential key codes */)]
	pub fn from_code(code: u32) -> Option<Self> {
		Some(match code {
			2..=11 => Self::NumberBar,
			16 => Self::S,
			17 => Self::T,
			18 => Self::P,
			19 => Self::H,
			20 => Self::Star,
			21 => Self::F,
			22 => Self::P2,
			23 => Self::L,
			24 => Self::T2,
			25 => Self::D,
			30 => Self::S,
			31 => Self::K,
			32 => Self::W,
			33 => Self::R,
			34 => Self::Star,
			35 => Self::R2,
			36 => Self::B,
			37 => Self::G,
			38 => Self::S2,
			39 => Self::Z,
			46 => Self::A,
			47 => Self::O,
			48 => Self::E,
			49 => Self::U,
			_ => return None,
		})
	}

	pub fn to_char(self) -> char {
		match self {
			Self::NumberBar => '#',
			Self::S | Self::S2 => 'S',
			Self::T | Self::T2 => 'T',
			Self::K => 'K',
			Self::P | Self::P2 => 'P',
			Self::W => 'W',
			Self::H => 'H',
			Self::R | Self::R2 => 'R',
			Self::A => 'A',
			Self::O => 'O',
			Self::Star => '*',
			Self::E => 'E',
			Self::U => 'U',
			Self::F => 'F',
			Self::B => 'B',
			Self::L => 'L',
			Self::G => 'G',
			Self::D => 'D',
			Self::Z => 'Z',
		}
	}

	pub fn other(self) -> Option<Self> {
		macro_rules! make {
			($($a:ident <=> $b:ident),* $(,)?) => {
				Some(match self {
					$(
						Self::$a => Self::$b,
						Self::$b => Self::$a,
					)*
					_ => return None,
				})
			};
		}

		make! {
			R <=> R2,
			P <=> P2,
			S <=> S2,
			T <=> T2,
		}
	}

	pub fn other_before(self) -> Option<Self> {
		self.other().filter(|&other| other < self)
	}

	pub fn other_after(self) -> Option<Self> {
		self.other().filter(|&other| other > self)
	}
}

impl Display for Key {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		Keys::from(*self).fmt(formatter)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keys(u32);

impl Keys {
	pub const fn empty() -> Self {
		Self(0)
	}

	#[allow(clippy::cast_possible_truncation /* false positive; can't use `try_from` in `const fn` */)]
	pub const fn all() -> Self {
		Self((1 << Key::ALL.len() as u32) - 1)
	}

	pub const fn single(key: Key) -> Self {
		Self(1 << key as u32)
	}

	pub const fn is_empty(self) -> bool {
		self.0 == 0
	}

	pub const fn bits(self) -> u32 {
		self.0
	}

	pub const fn contains(self, key: Key) -> bool {
		self.0 & Keys::single(key).0 > 0
	}
}

#[test]
fn test_all() {
	assert_eq!(Keys::all().into_iter().collect::<Vec<_>>(), Key::ALL);
}

impl From<Key> for Keys {
	fn from(key: Key) -> Self {
		Self::single(key)
	}
}

impl Default for Keys {
	fn default() -> Self {
		Self::empty()
	}
}

macro_rules! bit_traits {
	($($trait:ident::$method:ident => $assign_trait:ident::$assign_method:ident),* $(,)?) => {
		$(
			impl $trait for Keys {
				type Output = Self;

				fn $method(self, rhs: Self) -> Self {
					Self($trait::$method(self.0, rhs.0))
				}
			}

			impl $trait<Key> for Keys {
				type Output = Self;

				fn $method(self, rhs: Key) -> Self {
					<Keys as $trait>::$method(self, rhs.into())
				}
			}

			impl $trait<Keys> for Key {
				type Output = Keys;

				fn $method(self, rhs: Keys) -> Keys {
					<Keys as $trait>::$method(self.into(), rhs)
				}
			}

			impl $trait for Key {
				type Output = Keys;

				fn $method(self, rhs: Self) -> Keys {
					<Keys as $trait>::$method(self.into(), rhs.into())
				}
			}

			impl $assign_trait for Keys {
				fn $assign_method(&mut self, rhs: Self) {
					*self = $trait::$method(*self, rhs);
				}
			}

			impl $assign_trait<Key> for Keys {
				fn $assign_method(&mut self, rhs: Key) {
					<Keys as $assign_trait>::$assign_method(self, rhs.into());
				}
			}

		)*
	};
}

bit_traits! {
	BitOr::bitor => BitOrAssign::bitor_assign,
	BitAnd::bitand => BitAndAssign::bitand_assign,
	BitXor::bitxor => BitXorAssign::bitxor_assign,
}

impl Not for Keys {
	type Output = Self;

	fn not(self) -> Self {
		// We don't want to expose unmapped bits to users, so AND with `Self::all()`.
		Self(!self.0) & Self::all()
	}
}

impl Not for Key {
	type Output = Keys;

	fn not(self) -> Keys {
		!Keys::from(self)
	}
}

impl Display for Keys {
	fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
		if formatter.alternate() {
			for possible in Self::all() {
				let ch = if self.contains(possible) {
					possible.to_char()
				} else {
					' '
				};
				formatter.write_char(ch)?;
				formatter.write_char(' ')?;
			}
		} else {
			for key in self {
				let needs_dash = {
					let second = key;
					key.other_before().map_or(false, |first| {
						!self.into_iter().any(|key| key >= first && key < second)
					})
				};
				if needs_dash {
					formatter.write_str("-")?;
				}
				key.to_char().fmt(formatter)?;
			}
		}
		Ok(())
	}
}

#[test]
fn test_display() {
	assert_eq!((Key::S | Key::S2).to_string(), "SS");
	assert_eq!((Key::S2).to_string(), "-S");
	assert_eq!((Key::A | Key::O | Key::S2).to_string(), "AOS");
}

impl FromIterator<Key> for Keys {
	fn from_iter<I: IntoIterator<Item = Key>>(iter: I) -> Self {
		iter.into_iter().fold(Keys::empty(), Keys::bitor)
	}
}

impl IntoIterator for Keys {
	type Item = Key;
	type IntoIter = IntoIter;

	fn into_iter(self) -> IntoIter {
		IntoIter(self.0)
	}
}

impl IntoIterator for &Keys {
	type Item = Key;
	type IntoIter = IntoIter;

	fn into_iter(self) -> IntoIter {
		IntoIter(self.0)
	}
}

pub struct IntoIter(u32);

impl Iterator for IntoIter {
	type Item = Key;

	fn next(&mut self) -> Option<Key> {
		let first_bit = self.0.trailing_zeros();
		let item = first_bit.try_into().ok()?;
		self.0 &= !(1 << first_bit);
		Some(item)
	}
}

#[test]
fn test_iterator() {
	assert_eq!(
		(Key::A | Key::O | Key::S2).into_iter().collect::<Vec<_>>(),
		[Key::A, Key::O, Key::S2],
	);
}
