use std::{cmp::Ordering, io};

use crate::{heap::Offset, Decode, DecodeFromHeap, EncodeSized, HeapSection, Section};

mod cache;
mod page;

pub use cache::{Cache, Ref};
pub use page::Page;
use parking_lot::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error(transparent)]
	IO(#[from] io::Error),

	#[error("out of memory")]
	OutOfMemory,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
	pub page_len: u32,
	pub first_page_offset: u32,
}

pub struct Cursor<R> {
	input: R,
	current_offset: u32,
	options: Options,
}

impl<R: io::Seek> Cursor<R> {
	pub fn seek(&mut self, offset: u32) -> io::Result<()> {
		self.input.seek(io::SeekFrom::Start(offset as u64))?;
		self.current_offset = offset;
		Ok(())
	}
}

impl<R: io::Read> Cursor<R> {
	pub fn read(&mut self, bytes: &mut [u8]) -> io::Result<()> {
		self.input.read_exact(bytes)?;
		Ok(())
	}

	/// Decodes arbitrary data from the heap.
	pub fn decode_from_heap<C, T: Decode<C>>(
		&mut self,
		context: &mut C,
		heap: &HeapSection,
		offset: Offset,
	) -> io::Result<T>
	where
		R: io::Seek,
	{
		let saved_offset = self.current_offset;
		self.seek(
			self.options.first_page_offset
				+ heap.page_offset * self.options.page_len
				+ offset.unwrap(),
		)?;
		let t = T::decode(self, context)?;
		self.seek(saved_offset)?;
		Ok(t)
	}

	/// Read arbitrary data from the heap.
	pub fn read_from_heap(
		&mut self,
		heap: &HeapSection,
		offset: Offset,
		bytes: &mut [u8],
	) -> io::Result<()>
	where
		R: io::Seek,
	{
		let saved_offset = self.current_offset;
		self.seek(
			self.options.first_page_offset
				+ heap.page_offset * self.options.page_len
				+ offset.unwrap(),
		)?;
		self.input.read_exact(bytes)?;
		self.seek(saved_offset)?;
		Ok(())
	}
}

pub struct Reader<R> {
	cursor: Mutex<Cursor<R>>,
	options: Options,
}

impl<R: io::Seek + io::Read> Reader<R> {
	pub fn get_page<'a, C, T: EncodeSized + DecodeFromHeap<C>>(
		&self,
		section: &Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: &HeapSection,
		page_index: u32,
	) -> Result<Ref<'a, T>, Error> {
		cache.get_or_insert(page_index, |page| {
			let offset = self.options.first_page_offset
				+ section.offset_of_page(self.options.page_len, page_index);
			let entry_count = section.page_size(self.options.page_len, page_index);

			let mut cursor = self.cursor.lock();
			cursor.seek(offset)?;
			for _ in 0..entry_count {
				page.push(T::decode_from_heap(&mut cursor, context, heap)?)
			}

			Ok(())
		})
	}

	pub fn get<'a, C, T: EncodeSized + DecodeFromHeap<C>>(
		&self,
		section: &Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: &HeapSection,
		entry_index: u32,
	) -> Result<Ref<'a, T, T>, Error> {
		let (page_index, i) = section.page_of_entry(self.options.page_len, entry_index);
		let page = self.get_page(section, cache, context, heap, page_index)?;
		Ok(page.map(|page| page.get(i).unwrap()))
	}

	pub fn binary_search_by_key<'a, C, T: EncodeSized + DecodeFromHeap<C>, K: Ord>(
		&self,
		section: &Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: &HeapSection,
		key: &K,
		f: impl Fn(&T) -> &K,
	) -> Result<Option<Ref<'a, T, T>>, Error> {
		let mut min = 0;
		let mut max = section.page_count(self.options.page_len);

		let mut page_index = max / 2;

		while page_index < max {
			let page = self.get_page(section, cache, context, heap, page_index)?;
			match page.binary_search_by_key(key, &f) {
				Ok(i) => return Ok(Some(page.map(|page| page.get(i).unwrap()))),
				Err(Ordering::Greater) => {
					max = page_index;
				}
				Err(Ordering::Less) => {
					min = page_index;
				}
				Err(Ordering::Equal) => break,
			}

			page_index = (min + max) / 2;
		}

		Ok(None)
	}

	/// Decodes arbitrary data from the heap.
	pub fn decode_from_heap<C, T: Decode<C>>(
		&mut self,
		context: &mut C,
		heap: &HeapSection,
		offset: Offset,
	) -> io::Result<T> {
		let mut cursor = self.cursor.lock();
		cursor.decode_from_heap(context, heap, offset)
	}
}
