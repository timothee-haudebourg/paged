use std::io;

use crate::{
	encode::{Encode, EncodeSized},
	reader,
	utils::CeilingDiv,
	Decode, DecodeFromHeap, EncodeOnHeap,
};

pub struct Heap {
	data: Vec<u8>,
}

impl Heap {
	pub fn new() -> Self {
		Self { data: Vec::new() }
	}

	pub fn len(&self) -> u32 {
		self.data.len() as u32
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.data
	}

	pub fn insert<C>(
		&mut self,
		context: &C,
		value: &(impl ?Sized + Encode<C>),
	) -> io::Result<Offset> {
		let offset = Offset(self.data.len() as u32);
		let mut writer = Writer {
			data: &mut self.data,
		};
		value.encode(context, &mut writer)?;
		Ok(offset)
	}

	pub fn page_count(&self, page_len: u32) -> u32 {
		self.len().ceiling_div(page_len)
	}

	pub fn padding(&self, page_len: u32) -> u32 {
		let shift = self.len() % page_len;
		if shift == 0 {
			0
		} else {
			page_len - shift
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Offset(u32);

impl Offset {
	pub fn unwrap(self) -> u32 {
		self.0
	}

	pub fn sized(self, len: u32) -> Entry {
		Entry { offset: self, len }
	}
}

impl<C> Encode<C> for Offset {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		self.0.encode(context, output)
	}
}

impl EncodeSized for Offset {
	const ENCODED_SIZE: u32 = u32::ENCODED_SIZE;
}

impl<C> Decode<C> for Offset {
	fn decode<R: io::Read>(input: &mut reader::Cursor<R>, context: &mut C) -> io::Result<Self> {
		Ok(Self(u32::decode(input, context)?))
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
	pub offset: Offset,
	pub len: u32,
}

impl<C> Encode<C> for Entry {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		self.offset.encode(context, output)?;
		self.len.encode(context, output)?;
		Ok(Self::ENCODED_SIZE)
	}
}

impl EncodeSized for Entry {
	const ENCODED_SIZE: u32 = Offset::ENCODED_SIZE + u32::ENCODED_SIZE;
}

impl<C> Decode<C> for Entry {
	fn decode<R: io::Read>(input: &mut reader::Cursor<R>, context: &mut C) -> io::Result<Self> {
		Ok(Self {
			offset: Offset::decode(input, context)?,
			len: u32::decode(input, context)?,
		})
	}
}

pub struct Writer<'a> {
	data: &'a mut Vec<u8>,
}

impl<'a> io::Write for Writer<'a> {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.data.extend_from_slice(buf);
		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}

pub struct HeapSection {
	pub page_offset: u32,
	pub page_count: u32,
}

impl<C> Encode<C> for HeapSection {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		self.page_offset.encode(context, output)?;
		self.page_count.encode(context, output)?;
		Ok(Self::ENCODED_SIZE)
	}
}

impl<C> EncodeOnHeap<C> for HeapSection {
	fn encode_on_heap(
		&self,
		context: &C,
		_heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		self.encode(context, output)
	}
}

impl EncodeSized for HeapSection {
	const ENCODED_SIZE: u32 = u32::ENCODED_SIZE + u32::ENCODED_SIZE;
}

impl<C> Decode<C> for HeapSection {
	fn decode<R: io::Read>(input: &mut reader::Cursor<R>, context: &mut C) -> io::Result<Self> {
		Ok(Self {
			page_offset: u32::decode(input, context)?,
			page_count: u32::decode(input, context)?,
		})
	}
}

impl<C> DecodeFromHeap<C> for HeapSection {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut reader::Cursor<R>,
		context: &mut C,
		_heap: &HeapSection,
	) -> io::Result<Self> {
		Self::decode(input, context)
	}
}
