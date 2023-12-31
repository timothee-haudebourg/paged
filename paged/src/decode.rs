use std::io;

use crate::{heap, reader, EncodeSized, HeapSection};

pub trait Decode<C>: Sized {
	fn decode<R: io::Read>(input: &mut R, context: &mut C) -> io::Result<Self>;
}

macro_rules! decode_int {
	($($ty:ty),*) => {
		$(
			impl<C> Decode<C> for $ty {
				fn decode<R: io::Read>(
					input: &mut R,
					_context: &mut C
				) -> io::Result<Self> {
					let mut result = [0u8; std::mem::size_of::<$ty>()];
					input.read_exact(&mut result)?;
					Ok(Self::from_be_bytes(result))
				}
			}

			impl<C> DecodeFromHeap<C> for $ty {
				fn decode_from_heap<R: io::Seek + io::Read>(
					input: &mut reader::Cursor<R>,
					context: &mut C,
					_heap: HeapSection
				) -> io::Result<Self> {
					Self::decode(input, context)
				}
			}
		)*
	};
}

decode_int!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

pub trait DecodeFromHeap<C = ()>: Sized {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		heap: HeapSection,
	) -> io::Result<Self>;
}

impl<C> DecodeFromHeap<C> for String {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		heap: HeapSection,
	) -> io::Result<Self> {
		let entry = heap::Entry::decode(input, context)?;
		let mut bytes = Vec::new();
		bytes.resize(entry.len as usize, 0u8);
		input.read_from_heap(heap, entry.offset, bytes.as_mut_slice())?;
		String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData.into())
	}
}

fn pad(input: &mut impl io::Read, len: u32) -> io::Result<()> {
	let mut buffer = [0u8; 1];
	for _ in 0..len {
		input.read_exact(&mut buffer)?;
	}

	Ok(())
}

impl<C, T: EncodeSized + Decode<C>> Decode<C> for Option<T> {
	fn decode<R: io::Read>(input: &mut R, context: &mut C) -> io::Result<Self> {
		let discriminant = u8::decode(input, context)?;
		match discriminant {
			0 => {
				pad(input, T::ENCODED_SIZE)?;
				Ok(None)
			}
			1 => T::decode(input, context).map(Some),
			_ => Err(io::ErrorKind::InvalidData.into()),
		}
	}
}

impl<C, T: EncodeSized + DecodeFromHeap<C>> DecodeFromHeap<C> for Option<T> {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		heap: HeapSection,
	) -> io::Result<Self> {
		let discriminant = u8::decode(input, context)?;
		match discriminant {
			0 => {
				input.pad(T::ENCODED_SIZE)?;
				Ok(None)
			}
			1 => T::decode_from_heap(input, context, heap).map(Some),
			_ => Err(io::ErrorKind::InvalidData.into()),
		}
	}
}

impl<C, T: Decode<C>> DecodeFromHeap<C> for Vec<T> {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		heap: HeapSection,
	) -> io::Result<Self> {
		let entry = heap::Entry::decode(input, context)?;
		let mut result = Vec::with_capacity(entry.len as usize);

		for _ in 0..entry.len {
			result.push(input.decode_from_heap(context, heap, entry.offset)?)
		}

		Ok(result)
	}
}

impl<C, T1: Decode<C>, T2: Decode<C>> Decode<C> for (T1, T2) {
	fn decode<R: io::Read>(input: &mut R, context: &mut C) -> io::Result<Self> {
		let t1 = T1::decode(input, context)?;
		let t2 = T2::decode(input, context)?;
		Ok((t1, t2))
	}
}

impl<C, T1: DecodeFromHeap<C>, T2: DecodeFromHeap<C>> DecodeFromHeap<C> for (T1, T2) {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		heap: HeapSection,
	) -> io::Result<Self> {
		let t1 = T1::decode_from_heap(input, context, heap)?;
		let t2 = T2::decode_from_heap(input, context, heap)?;
		Ok((t1, t2))
	}
}
