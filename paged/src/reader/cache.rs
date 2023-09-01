use parking_lot::RwLock;
use sharded_slab::{pool, Pool};
use std::collections::HashMap;
use std::ops::Deref;

use super::{Error, Page};

pub struct Cache<T> {
	index: RwLock<HashMap<u32, usize>>,
	pool: Pool<Page<T>>,
}

impl<T> Cache<T> {
	fn index_of(&self, page_index: u32) -> Option<usize> {
		self.index.read().get(&page_index).copied()
	}

	pub fn get(&self, page_index: u32) -> Option<Ref<T>> {
		self.index_of(page_index)
			.map(|i| Ref::new(self.pool.get(i).unwrap()))
	}

	pub fn set(
		&self,
		page_index: u32,
		init: impl FnOnce(&mut Page<T>) -> Result<(), Error>,
	) -> Result<Ref<T>, Error> {
		let mut result = Ok(());
		let i = self
			.pool
			.create_with(|page| result = init(page))
			.ok_or(Error::OutOfMemory)?;

		match result {
			Ok(()) => {
				self.index.write().insert(page_index, i);
				Ok(Ref::new(self.pool.get(i).unwrap()))
			}
			Err(e) => {
				self.pool.clear(i);
				Err(e)
			}
		}
	}

	pub fn get_or_insert(
		&self,
		page_index: u32,
		init: impl FnOnce(&mut Page<T>) -> Result<(), Error>,
	) -> Result<Ref<T>, Error> {
		match self.get(page_index) {
			Some(page) => Ok(page),
			None => self.set(page_index, init),
		}
	}
}

/// Page reference.
pub struct Ref<'a, T, U = Page<T>> {
	t: pool::Ref<'a, Page<T>>,
	u: &'a U,
}

impl<'a, T> Ref<'a, T> {
	fn new(t: pool::Ref<'a, Page<T>>) -> Self {
		Self::new_projection(t, |t| t)
	}
}

impl<'a, T, U> Ref<'a, T, U> {
	fn new_projection(t: pool::Ref<'a, Page<T>>, f: impl FnOnce(&Page<T>) -> &U) -> Self {
		let short_u: &U = f(&t);
		let u: &'a U = unsafe { std::mem::transmute(short_u) };

		Self { t, u }
	}

	pub fn map<V>(self, f: impl FnOnce(&U) -> &V) -> Ref<'a, T, V> {
		Ref {
			t: self.t,
			u: f(self.u),
		}
	}
}

impl<'a, T, U> Deref for Ref<'a, T, U> {
	type Target = U;

	fn deref(&self) -> &Self::Target {
		self.u
	}
}
