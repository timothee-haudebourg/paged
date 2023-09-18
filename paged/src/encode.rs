use std::io;

use crate::heap::{self, Heap};

pub trait Encode<C = ()> {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32>;
}

pub trait EncodeOnHeap<C = ()>: EncodeSized {
	fn encode_on_heap(
		&self,
		context: &C,
		heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32>;
}

pub trait EncodeSized {
	const ENCODED_SIZE: u32;
}

macro_rules! encode_int {
	($($ty:ty),*) => {
		$(
			impl<C> Encode<C> for $ty {
				fn encode(&self, _context: &C, output: &mut impl io::Write) -> io::Result<u32> {
					output.write_all(&self.to_be_bytes())?;
					Ok(Self::ENCODED_SIZE)
				}
			}

			impl<C> EncodeOnHeap<C> for $ty {
				fn encode_on_heap(&self, _context: &C, _heap: &mut Heap, output: &mut impl io::Write) -> io::Result<u32> {
					output.write_all(&self.to_be_bytes())?;
					Ok(Self::ENCODED_SIZE)
				}
			}

			impl EncodeSized for $ty {
				const ENCODED_SIZE: u32 = std::mem::size_of::<$ty>() as u32;
			}
		)*
	};
}

encode_int!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

pub fn encode_string_on_heap(
	heap: &mut Heap,
	output: &mut impl io::Write,
	str: &str,
) -> io::Result<u32> {
	let entry = heap.insert(&(), str)?.sized(str.len() as u32);
	entry.encode(&(), output)
}

impl<C> Encode<C> for str {
	fn encode(&self, _context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		output.write_all(self.as_bytes())?;
		Ok(self.len() as u32)
	}
}

impl<C> EncodeOnHeap<C> for String {
	fn encode_on_heap(
		&self,
		_context: &C,
		heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		encode_string_on_heap(heap, output, self.as_str())
	}
}

impl EncodeSized for String {
	const ENCODED_SIZE: u32 = heap::Entry::ENCODED_SIZE;
}

fn pad(output: &mut impl io::Write, len: u32) -> io::Result<u32> {
	for _ in 0..len {
		0u8.encode(&(), output)?;
	}
	Ok(len)
}

impl<T: EncodeSized> EncodeSized for Option<T> {
	const ENCODED_SIZE: u32 = 1 + T::ENCODED_SIZE;
}

impl<C, T: EncodeSized + Encode<C>> Encode<C> for Option<T> {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		match self {
			Self::None => {
				Ok(0u8.encode(context, output)? + pad(output, T::ENCODED_SIZE)?)
			}
			Self::Some(t) => {
				Ok(1u8.encode(context, output)? + t.encode(context, output)?)
			}
		}
	}
}

impl<C, T: EncodeOnHeap<C>> EncodeOnHeap<C> for Option<T> {
	fn encode_on_heap(
		&self,
		context: &C,
		heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		match self {
			Self::None => {
				Ok(0u8.encode(context, output)? + pad(output, T::ENCODED_SIZE)?)
			}
			Self::Some(t) => {
				Ok(1u8.encode(context, output)? + t.encode_on_heap(context, heap, output)?)
			}
		}
	}
}

impl<C, T: Encode<C>> Encode<C> for [T] {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		let mut len = 0;
		for t in self {
			len += t.encode(context, output)?;
		}
		Ok(len)
	}
}

impl<C, T: Encode<C>> EncodeOnHeap<C> for Vec<T> {
	fn encode_on_heap(
		&self,
		context: &C,
		heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		let entry = heap
			.insert(context, self.as_slice())?
			.sized(self.len() as u32);
		entry.encode(context, output)
	}
}

impl<T> EncodeSized for Vec<T> {
	const ENCODED_SIZE: u32 = heap::Entry::ENCODED_SIZE;
}

impl<T1: EncodeSized, T2: EncodeSized> EncodeSized for (T1, T2) {
	const ENCODED_SIZE: u32 = T1::ENCODED_SIZE + T2::ENCODED_SIZE;
}

impl<C, T1: Encode<C>, T2: Encode<C>> Encode<C> for (T1, T2) {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		let a = self.0.encode(context, output)?;
		let b = self.1.encode(context, output)?;
		Ok(a + b)
	}
}

impl<C, T1: EncodeOnHeap<C>, T2: EncodeOnHeap<C>> EncodeOnHeap<C> for (T1, T2) {
	fn encode_on_heap(
		&self,
		context: &C,
		heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		let a = self.0.encode_on_heap(context, heap, output)?;
		let b = self.0.encode_on_heap(context, heap, output)?;
		Ok(a + b)
	}
}
