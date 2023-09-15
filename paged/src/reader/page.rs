use std::{cmp::Ordering, marker::PhantomData};

use super::cache::{Binder, UnboundRef, UnboundSliceIter};

pub struct Page<T> {
	entries: Vec<T>,
}

impl<T> Default for Page<T> {
	fn default() -> Self {
		Self {
			entries: Vec::new(),
		}
	}
}

impl<T> Page<T> {
	pub fn get(&self, i: u32) -> Option<&T> {
		self.entries.get(i as usize)
	}

	pub fn iter(&self) -> Iter<T> {
		self.entries.iter()
	}

	pub fn binary_search_by_key<C>(
		&self,
		context: &C,
		f: impl Fn(&T, &C) -> Ordering,
	) -> Result<u32, Ordering> {
		if self.entries.is_empty() {
			Err(Ordering::Equal)
		} else if f(self.entries.first().unwrap(), context).is_gt() {
			Err(Ordering::Greater)
		} else if f(self.entries.last().unwrap(), context).is_lt() {
			Err(Ordering::Less)
		} else {
			match self.entries.binary_search_by(|t| f(t, context)) {
				Ok(i) => Ok(i as u32),
				Err(_) => Err(Ordering::Equal),
			}
		}
	}

	pub fn push(&mut self, entry: T) {
		self.entries.push(entry)
	}
}

impl<T> sharded_slab::Clear for Page<T> {
	fn clear(&mut self) {
		self.entries.clear()
	}
}

pub type Iter<'a, T> = std::slice::Iter<'a, T>;

pub type UnboundIter<T> = UnboundSliceIter<T>;

pub struct GetEntryBinder<T> {
	index: u32,
	t: PhantomData<T>,
}

impl<T> GetEntryBinder<T> {
	pub fn new(index: u32) -> Self {
		Self {
			index,
			t: PhantomData,
		}
	}
}

impl<'a, T> Binder<'a, UnboundRef<Page<T>>, UnboundRef<T>> for GetEntryBinder<T> {
	fn bind<'t>(self, page: &'t Page<T>) -> &'t T
	where
		'a: 't,
	{
		page.get(self.index).unwrap()
	}
}

pub struct IterBinder<T>(PhantomData<T>);

impl<T> Default for IterBinder<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T> IterBinder<T> {
	pub fn new() -> Self {
		Self(PhantomData)
	}
}

impl<'a, T> Binder<'a, UnboundRef<Page<T>>, UnboundIter<T>> for IterBinder<T> {
	fn bind<'t>(self, page: &'t Page<T>) -> Iter<'t, T>
	where
		'a: 't,
	{
		page.iter()
	}
}
