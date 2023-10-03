//! This library provides a simple binary data format to organize large lists of data for random read-only access.
//!
//! ## Anatomy of a paged file
//!
//! Data is organized in sections, pages and entries. Each file can include multiple sections, each section include multiple pages, and each page multiple entries. The first section/page may start at any given offset to give room to a potential file header. The file may also include one or more heap sections storing dynamically sized data.
//!
//! A typical paged file will look like this:
//! ```
//! ┏━━━━━━━━━━━┓
//! ┃  Header   ┃
//! ┃           ┃
//! ┗━━━━━━━━━━━┛
//! ┏━━━━━━━━━━━┓
//! ┃ Section 1 ┃
//! ┃┌─────────┐┃
//! ┃│ Page 1  │┃
//! ┃├─────────┤┃
//! ┃│ Entry 1 │┃
//! ┃│   ...   │┃
//! ┃│ Entry N │┃
//! ┃└─────────┘┃
//! ┃    ...    ┃
//! ┃┌─────────┐┃
//! ┃│ Page M  │┃
//! ┃├─────────┤┃
//! ┃│ Entry 1 │┃
//! ┃│   ...   │┃
//! ┃│ Entry N │┃
//! ┃└─────────┘┃
//! ┗━━━━━━━━━━━┛
//!      ...
//! ┏━━━━━━━━━━━┓
//! ┃ Section P ┃
//! ┃┌─────────┐┃
//! ┃│ Page 1  │┃
//! ┃├─────────┤┃
//! ┃│ Entry 1 │┃
//! ┃│   ...   │┃
//! ┃│ Entry N │┃
//! ┃└─────────┘┃
//! ┃    ...    ┃
//! ┃┌─────────┐┃
//! ┃│ Page M  │┃
//! ┃├─────────┤┃
//! ┃│ Entry 1 │┃
//! ┃│   ...   │┃
//! ┃│ Entry N │┃
//! ┃└─────────┘┃
//! ┗━━━━━━━━━━━┛
//! ┏━━━━━━━━━━━┓
//! ┃   Heap    ┃
//! ┃           ┃
//! ┃           ┃
//! ┃           ┃
//! ┃           ┃
//! ┃           ┃
//! ┃           ┃
//! ┗━━━━━━━━━━━┛
//! ```
//!
//! ### Entry
//!
//! The "entry" is the smallest unit of information in a file managed by this library. An entry represents any data whose type implements the `EncodeOnHeap` and `DecodeFromHeap` traits.
//!
//! ### Page
//!
//! A page is a list of entries of the same type. Every page of a file have the same byte length. All the entries in a page must have the same size. It is however possible for an entry to reference dynamically sized data living on a heap section.
//!
//! ### Sections
//!
//! A section is a list of pages of the same type. The size of a section is a multiple of the page length.
//!
//! ### Heaps
//!
//! A file may contain one or more heap sections. A heap stores dynamically sized data without any structure.
use std::io;
use std::ops::Deref;

#[cfg(feature = "derive")]
pub use paged_derive::Paged;

mod decode;
mod encode;
pub mod heap;
pub mod reader;
pub mod section;
pub mod utils;

pub use decode::*;
pub use encode::*;
pub use heap::{Heap, HeapSection};
pub use reader::*;
pub use section::Section;

pub fn no_context_mut() -> &'static mut () {
	unsafe { std::mem::transmute(&mut ()) }
}

pub struct Encoder<W> {
	output: W,
	page_len: u32,
	page_count: u32,
}

impl<W> Encoder<W> {
	pub fn new(output: W, page_len: u32) -> Self {
		Self {
			output,
			page_len,
			page_count: 0,
		}
	}

	pub fn begin_section<'h, T>(&mut self, heap: &'h mut Heap) -> section::Encoder<'_, 'h, W, T> {
		section::Encoder::new(self, heap, self.page_count)
	}

	pub fn end(self) -> W {
		self.output
	}

	pub fn section_from_iter<I: IntoIterator>(
		&mut self,
		heap: &mut Heap,
		items: I,
	) -> io::Result<Section<<I::Item as Deref>::Target>>
	where
		I::Item: Deref,
		<I::Item as Deref>::Target: Sized + EncodeOnHeap,
		W: io::Write + io::Seek,
	{
		let mut encoder = self.begin_section(heap);

		for item in items {
			encoder.push(&(), &*item)?
		}

		encoder.end()
	}

	pub fn section_from_iter_with<I: IntoIterator, C>(
		&mut self,
		heap: &mut Heap,
		context: &C,
		items: I,
	) -> io::Result<Section<<I::Item as Deref>::Target>>
	where
		I::Item: Deref,
		<I::Item as Deref>::Target: Sized + EncodeOnHeap<C>,
		W: io::Write + io::Seek,
	{
		let mut encoder = self.begin_section(heap);

		for item in items {
			encoder.push(context, &*item)?
		}

		encoder.end()
	}
}

impl<W: io::Seek> Encoder<W> {
	pub(crate) fn pad(&mut self, padding: u32) -> io::Result<()> {
		self.output.seek(io::SeekFrom::Current(padding as i64))?;
		Ok(())
	}

	pub fn add_heap(&mut self, heap: Heap) -> io::Result<HeapSection>
	where
		W: io::Write,
	{
		let page_offset = self.page_count;
		let page_count = heap.page_count(self.page_len);
		self.output.write_all(heap.as_bytes())?;
		self.pad(heap.padding(self.page_len))?;
		self.page_count += page_count;
		Ok(HeapSection {
			page_offset,
			page_count,
		})
	}
}
