use std::io;

pub use paged_derive::Paged;

mod decode;
mod encode;
pub mod heap;
pub mod reader;
pub mod section;
mod utils;

pub use decode::*;
pub use encode::*;
pub use heap::{Heap, HeapSection};
pub use section::Section;

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
