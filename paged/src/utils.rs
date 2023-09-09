use std::io;

use crate::{Encode, EncodeSized, Decode};

pub trait CeilingDiv {
	fn ceiling_div(self, other: Self) -> Self;
}

impl CeilingDiv for u32 {
	fn ceiling_div(self, other: Self) -> Self {
		(self + other - 1) / other
	}
}

pub const fn max(a: u32, b: u32) -> u32 {
	if a > b {
		a
	} else {
		b
	}
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Inline<T>(pub T);

impl<T> std::ops::Deref for Inline<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> std::ops::DerefMut for Inline<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<C, T: Encode<C>> Encode<C> for Inline<Vec<T>> {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		(self.0.len() as u32).encode(context, output)?;
		Ok(u32::ENCODED_SIZE + self.0.as_slice().encode(context, output)?)
	}
}

impl<C, T: Decode<C>> Decode<C> for Inline<Vec<T>> {
	fn decode<R: io::Read>(input: &mut R, context: &mut C) -> io::Result<Self> {
		let len = u32::decode(input, context)?;
		let mut result = Vec::with_capacity(len as usize);
		
		for _ in 0..len {
			result.push(T::decode(input, context)?)
		}
		
		Ok(Self(result))
	}
}