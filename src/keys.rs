use bitflags::bitflags;

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
	pub struct Keys: u32 {
		const NUMBER_BAR = 1 << 0;
		const S = 1 << 1;
		const T = 1 << 2;
		const K = 1 << 3;
		const P = 1 << 4;
		const W = 1 << 5;
		const H = 1 << 6;
		const R = 1 << 7;
		const A = 1 << 8;
		const O = 1 << 9;
		const STAR = 1 << 10;
		const E = 1 << 11;
		const U = 1 << 12;
		const F = 1 << 13;
		const R2 = 1 << 14;
		const P2 = 1 << 15;
		const B = 1 << 16;
		const L = 1 << 17;
		const G = 1 << 18;
		const T2 = 1 << 19;
		const S2 = 1 << 20;
		const D = 1 << 21;
		const Z = 1 << 22;
	}
}

impl Keys {
	pub fn from_code(code: u32) -> Option<Self> {
		Some(match code {
			2..=11 => Self::NUMBER_BAR,
			16 => Self::S,
			17 => Self::T,
			18 => Self::P,
			19 => Self::H,
			20 => Self::STAR,
			21 => Self::F,
			22 => Self::P2,
			23 => Self::L,
			24 => Self::T2,
			25 => Self::D,
			30 => Self::S,
			31 => Self::K,
			32 => Self::W,
			33 => Self::R,
			34 => Self::STAR,
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
}
