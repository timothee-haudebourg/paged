use std::{cmp::Ordering, io};

use crate::{
	heap::Offset, no_context_mut, Decode, DecodeFromHeap, EncodeSized, HeapSection, Section,
};

pub mod cache;
pub mod page;

pub use cache::{Cache, EntryRef, Ref, UnboundRef, UnboundSliceIter};
pub use page::Page;
use parking_lot::Mutex;

use self::page::GetEntryBinder;

// use self::cache::RefIntoIter;

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

	pub fn pad(&mut self, padding: u32) -> io::Result<()> {
		self.input.seek(io::SeekFrom::Current(padding as i64))?;
		self.current_offset += padding;
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
		heap: HeapSection,
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
		heap: HeapSection,
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

impl<R: io::Read> io::Read for Cursor<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let len = self.input.read(buf)?;
		self.current_offset += len as u32;
		Ok(len)
	}
}

pub struct Reader<R> {
	cursor: Mutex<Cursor<R>>,
	options: Options,
}

impl<R> Reader<R> {
	/// Creates a new reader.
	///
	/// It is assumed that the current input position is `first_page_offset`.
	pub fn new(input: R, page_len: u32, first_page_offset: u32) -> Self {
		let options = Options {
			page_len,
			first_page_offset,
		};

		Self {
			cursor: Mutex::new(Cursor {
				input,
				current_offset: first_page_offset,
				options,
			}),
			options,
		}
	}
}

impl<R: io::Seek + io::Read> Reader<R> {
	pub fn get_page<'a, C, T: EncodeSized + DecodeFromHeap<C>>(
		&self,
		section: Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: HeapSection,
		page_index: u32,
	) -> Result<Ref<'a, T>, Error> {
		let global_page_index = page_index + section.page_offset();
		cache.get_or_insert(global_page_index, |page| {
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
		section: Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: HeapSection,
		entry_index: u32,
	) -> Result<Option<Ref<'a, T, UnboundRef<T>>>, Error> {
		if entry_index < section.entry_count() {
			let (page_index, i) = section.page_of_entry(self.options.page_len, entry_index);
			let page = self.get_page(section, cache, context, heap, page_index)?;
			Ok(Some(page.map(GetEntryBinder::new(i))))
		} else {
			Ok(None)
		}
	}

	pub fn pages<'a, 'c, T: EncodeSized>(
		&'a self,
		section: Section<T>,
		cache: &'c Cache<T>,
		heap: HeapSection,
	) -> Pages<'a, 'c, R, T> {
		Pages::new(self, section, cache, heap)
	}

	pub fn iter<'a, 'c, T: EncodeSized>(
		&'a self,
		section: Section<T>,
		cache: &'c Cache<T>,
		heap: HeapSection,
	) -> Iter<'a, 'c, R, T> {
		Iter {
			pages: self.pages(section, cache, heap),
			current_page: None,
		}
	}

	pub fn binary_search_by_key<'a, C, T: EncodeSized + DecodeFromHeap<C>>(
		&self,
		section: Section<T>,
		cache: &'a Cache<T>,
		context: &mut C,
		heap: HeapSection,
		f: impl Fn(&T, &C) -> Ordering,
	) -> Result<Option<Ref<'a, T, UnboundRef<T>>>, Error> {
		let mut min = 0;
		let mut max = section.page_count(self.options.page_len);

		let mut page_index = max / 2;

		while page_index < max {
			let page = self.get_page(section, cache, context, heap, page_index)?;
			match page.binary_search_by_key(context, &f) {
				Ok(i) => return Ok(Some(page.map(GetEntryBinder::new(i)))),
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
		heap: HeapSection,
		offset: Offset,
	) -> io::Result<T> {
		let mut cursor = self.cursor.lock();
		cursor.decode_from_heap(context, heap, offset)
	}
}

pub struct Pages<'a, 'c, R, T> {
	reader: &'a Reader<R>,
	section: Section<T>,
	cache: &'c Cache<T>,
	heap: HeapSection,
	page_count: u32,
	page_index: u32,
}

impl<'a, 'c, R, T: EncodeSized> Pages<'a, 'c, R, T> {
	fn new(
		reader: &'a Reader<R>,
		section: Section<T>,
		cache: &'c Cache<T>,
		heap: HeapSection,
	) -> Self {
		let page_count = section.page_count(reader.options.page_len);

		Self {
			reader,
			section,
			cache,
			heap,
			page_count,
			page_index: 0,
		}
	}
}

impl<'a, 'c, R: io::Seek + io::Read, C, T: EncodeSized + DecodeFromHeap<C>> ContextualIterator<C>
	for Pages<'a, 'c, R, T>
{
	type Item = Result<Ref<'c, T>, Error>;

	fn next_with(&mut self, context: &mut C) -> Option<Self::Item> {
		if self.page_index < self.page_count {
			match self.reader.get_page(
				self.section,
				self.cache,
				context,
				self.heap,
				self.page_index,
			) {
				Ok(page) => {
					self.page_index += 1;
					Some(Ok(page))
				}
				Err(e) => Some(Err(e)),
			}
		} else {
			None
		}
	}
}

impl<'a, 'c, R: io::Seek + io::Read, T: EncodeSized + DecodeFromHeap> Iterator
	for Pages<'a, 'c, R, T>
{
	type Item = Result<Ref<'c, T>, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		self.next_with(no_context_mut())
	}
}

pub struct Iter<'a, 'c, R, T> {
	pages: Pages<'a, 'c, R, T>,
	current_page: Option<Ref<'c, T, page::UnboundIter<T>>>,
}

impl<'a, 'c, R: io::Seek + io::Read, C, T: EncodeSized + DecodeFromHeap<C>> ContextualIterator<C>
	for Iter<'a, 'c, R, T>
{
	type Item = Result<Ref<'c, T, UnboundRef<T>>, Error>;

	fn next_with(&mut self, context: &mut C) -> Option<Self::Item> {
		loop {
			match &mut self.current_page {
				Some(page) => match page.next() {
					Some(entry) => break Some(Ok(entry)),
					None => self.current_page = None,
				},
				None => match self.pages.next_with(context) {
					Some(Ok(page)) => self.current_page = Some(page.map(page::IterBinder::new())),
					Some(Err(e)) => break Some(Err(e)),
					None => break None,
				},
			}
		}
	}
}

impl<'a, 'c, R: io::Seek + io::Read, T: EncodeSized + DecodeFromHeap> Iterator
	for Iter<'a, 'c, R, T>
{
	type Item = Result<Ref<'c, T, UnboundRef<T>>, Error>;

	fn next(&mut self) -> Option<Self::Item> {
		self.next_with(no_context_mut())
	}
}

pub trait ContextualIterator<C> {
	type Item;

	fn next_with(&mut self, context: &mut C) -> Option<Self::Item>;
}
