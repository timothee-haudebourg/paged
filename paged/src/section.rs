use std::{io, marker::PhantomData};

use crate::{
	encode::{Encode, EncodeSized},
	utils::CeilingDiv,
	Decode, DecodeFromHeap, EncodeOnHeap, Heap,
};

pub struct Section<T> {
	page_offset: u32,
	entry_count: u32,
	t: PhantomData<T>,
}

impl<T> Section<T> {
	pub fn offset_of_page(&self, page_len: u32, i: u32) -> u32 {
		(self.page_offset + i) * page_len
	}
}

impl<T: EncodeSized> Section<T> {
	pub fn page_count(&self, page_len: u32) -> u32 {
		let entries_per_page = page_len / T::ENCODED_SIZE;
		self.entry_count.ceiling_div(entries_per_page)
	}

	pub fn page_size(&self, page_len: u32, i: u32) -> u32 {
		let entries_per_page = page_len / T::ENCODED_SIZE;
		let past_entry_count = entries_per_page * i;
		let rest_entry_count = self.entry_count - past_entry_count;
		std::cmp::min(entries_per_page, rest_entry_count)
	}

	pub fn page_of_entry(&self, page_len: u32, i: u32) -> (u32, u32) {
		let entries_per_page = page_len / T::ENCODED_SIZE;
		let page = i / entries_per_page;
		let local_i = i % entries_per_page;
		(page, local_i)
	}
}

impl<C, T> Encode<C> for Section<T> {
	fn encode(&self, context: &C, output: &mut impl io::Write) -> io::Result<u32> {
		self.page_offset.encode(context, output)?;
		self.entry_count.encode(context, output)?;
		Ok(Self::ENCODED_SIZE)
	}
}

impl<C, T> EncodeOnHeap<C> for Section<T> {
	fn encode_on_heap(
		&self,
		context: &C,
		_heap: &mut Heap,
		output: &mut impl io::Write,
	) -> io::Result<u32> {
		Self::encode(&self, context, output)
	}
}

impl<T> EncodeSized for Section<T> {
	const ENCODED_SIZE: u32 = u32::ENCODED_SIZE + u32::ENCODED_SIZE;
}

impl<C, T> DecodeFromHeap<C> for Section<T> {
	fn decode_from_heap<R: io::Seek + io::Read>(
		input: &mut crate::reader::Cursor<R>,
		context: &mut C,
		_heap: &crate::HeapSection,
	) -> io::Result<Self> {
		Self::decode(input, context)
	}
}

impl<C, T> Decode<C> for Section<T> {
	fn decode<R: io::Read>(
		input: &mut crate::reader::Cursor<R>,
		context: &mut C,
	) -> io::Result<Self> {
		Ok(Self {
			page_offset: u32::decode(input, context)?,
			entry_count: u32::decode(input, context)?,
			t: PhantomData,
		})
	}
}

pub struct Encoder<'a, 'h, W, T> {
	encoder: &'a mut super::Encoder<W>,
	heap: &'h mut Heap,
	page_offset: u32,
	len: u32,
	entry_count: u32,
	empty_page: bool,
	t: PhantomData<T>,
}

impl<'a, 'h, W, T> Encoder<'a, 'h, W, T> {
	pub(crate) fn new(
		encoder: &'a mut super::Encoder<W>,
		heap: &'h mut Heap,
		page_offset: u32,
	) -> Self {
		Self {
			encoder,
			heap,
			page_offset,
			len: 0,
			entry_count: 0,
			empty_page: true,
			t: PhantomData,
		}
	}

	pub fn page_count(&self) -> u32 {
		self.len.ceiling_div(self.encoder.page_len)
	}

	fn padding(&self) -> u32 {
		let shift = self.len % self.encoder.page_len;
		if shift == 0 {
			0
		} else {
			self.encoder.page_len - shift
		}
	}
}

impl<'a, 'h, W, T: EncodeSized> Encoder<'a, 'h, W, T> {
	pub fn end(self) -> Section<T> {
		Section {
			page_offset: self.page_offset,
			entry_count: self.entry_count,
			t: PhantomData,
		}
	}
}

impl<'a, 'h, W: io::Write + io::Seek, T> Encoder<'a, 'h, W, T> {
	pub fn push<C>(&mut self, context: &C, value: &T) -> io::Result<()>
	where
		T: EncodeOnHeap<C>,
	{
		let len = value.encode_on_heap(context, self.heap, &mut self.encoder.output)?;

		if self.empty_page {
			self.encoder.page_count += 1;
			self.empty_page = false;
		}

		self.len += len;
		self.entry_count += 1;

		let padding = self.padding();
		if padding < T::ENCODED_SIZE {
			self.encoder.pad(padding)?;
			self.len += padding;
			self.empty_page = true
		}

		Ok(())
	}
}
