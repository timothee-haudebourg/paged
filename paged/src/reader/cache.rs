use educe::Educe;
use parking_lot::RwLock;
use sharded_slab::{pool, Pool};
use std::marker::PhantomData;
use std::{collections::HashMap, sync::Arc, ops::Deref};

use crate::ContextualIterator;

use super::{Error, Page};

#[derive(Educe)]
#[educe(Default)]
pub struct Cache<T> {
	index: RwLock<HashMap<u32, usize>>,
	pool: Pool<Page<T>>,
}

impl<T> Cache<T> {
	fn index_of(&self, global_page_index: u32) -> Option<usize> {
		self.index.read().get(&global_page_index).copied()
	}

	pub fn get(&self, global_page_index: u32) -> Option<Ref<T>> {
		self.index_of(global_page_index)
			.map(|i| Ref::new(self.pool.get(i).unwrap()))
	}

	pub fn set(
		&self,
		global_page_index: u32,
		init: impl FnOnce(&mut Page<T>) -> Result<(), Error>,
	) -> Result<Ref<T>, Error> {
		let mut result = Ok(());
		let i = self
			.pool
			.create_with(|page| result = init(page))
			.ok_or(Error::OutOfMemory)?;

		match result {
			Ok(()) => {
				self.index.write().insert(global_page_index, i);
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
		global_page_index: u32,
		init: impl FnOnce(&mut Page<T>) -> Result<(), Error>,
	) -> Result<Ref<T>, Error> {
		match self.get(global_page_index) {
			Some(page) => Ok(page),
			None => self.set(global_page_index, init),
		}
	}
}

pub trait Unbound {
	type Bound<'a> where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b> where Self: 'a + 'b;
}

pub trait UnboundIterator {
	type UnboundItem: Unbound;

	type Bound<'a>: 'a + Iterator<Item = <Self::UnboundItem as Unbound>::Bound<'a>> where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b>;
}

impl<T: UnboundIterator> Unbound for T {
	type Bound<'a> = <T as UnboundIterator>::Bound<'a> where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b> where Self: 'a + 'b {
		<T as UnboundIterator>::transmute_lifetime(value)
	}
}

pub trait UnboundContextualIterator<C = ()>: Unbound {
	type UnboundItem: Unbound;
}

impl<T: UnboundIterator> UnboundContextualIterator for T {
	type UnboundItem = <Self as UnboundIterator>::UnboundItem;
}

pub struct UnboundRef<T>(PhantomData<T>);

impl<T> Unbound for UnboundRef<T> {
	type Bound<'a> = &'a T where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b> where Self: 'a + 'b {
		std::mem::transmute(value)
	}
}

pub struct UnboundOwned<T>(PhantomData<T>);

impl<T> Unbound for UnboundOwned<T> {
	type Bound<'a> = T where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b> where Self: 'a + 'b {
		value
	}
}

pub trait Binder<'a, T: Unbound, U: Unbound> {
	fn bind<'t>(self, t: T::Bound<'t>) -> U::Bound<'t> where 'a: 't;
}

impl<'a, T, U: Unbound, F> Binder<'a, UnboundRef<T>, U> for F
where
	F: for<'b> FnOnce(&'b T) -> U::Bound<'b>
{
	fn bind<'t>(self, t: &'t T) -> U::Bound<'t> where 'a: 't {
		(self)(t)
	}
}

pub struct IdentityBinder;

impl<'a, T: Unbound> Binder<'a, T, T> for IdentityBinder {
	fn bind<'t>(self, t: T::Bound<'t>) -> T::Bound<'t> where 'a: 't {
		t
	}
}

pub struct UnboundSliceIter<T>(PhantomData<T>);

impl<T> UnboundIterator for UnboundSliceIter<T> {
	type UnboundItem = UnboundRef<T>;

	type Bound<'a> = std::slice::Iter<'a, T> where Self: 'a;

	unsafe fn transmute_lifetime<'a, 'b>(value: Self::Bound<'a>) -> Self::Bound<'b> {
		std::mem::transmute(value)
	}
}

/// Page reference.
#[derive(Educe)]
#[educe(Clone(bound = "for<'t> U::Bound<'t>: Clone"))]
pub struct Ref<'a, T, U: 'a + Unbound = UnboundRef<Page<T>>> {
	t: Arc<pool::Ref<'a, Page<T>>>,
	u: U::Bound<'a>,
}

pub type EntryRef<'a, T> = Ref<'a, T, UnboundRef<T>>;

impl<'a, T> Ref<'a, T> {
	fn new(t: pool::Ref<'a, Page<T>>) -> Self {
		Self::new_projection(t, IdentityBinder)
	}
}

impl<'a, T, U: Unbound> Ref<'a, T, U> {
	fn new_projection(page: pool::Ref<'a, Page<T>>, binder: impl Binder<'a, UnboundRef<Page<T>>, U>) -> Self {
		let u: U::Bound<'a> = unsafe { U::transmute_lifetime(binder.bind(&page)) };
		Self { t: Arc::new(page), u }
	}

	pub fn map<V: Unbound>(self, binder: impl Binder<'a, U, V>) -> Ref<'a, T, V> {
		Ref {
			t: self.t,
			u: binder.bind(self.u)
		}
	}

	pub fn unwrap(self) -> U::Bound<'static> where for<'t> U::Bound<'t>: 'static {
		unsafe {
			U::transmute_lifetime(self.u)
		}
	}
}

pub struct CloneBinder;

impl<'a, T: Clone> Binder<'a, UnboundRef<T>, UnboundOwned<T>> for CloneBinder {
	fn bind<'t>(self, t: &'t T) -> T where 'a: 't {
		t.clone()
	}
}

pub struct CopyBinder;

impl<'a, T: Copy> Binder<'a, UnboundRef<T>, UnboundOwned<T>> for CopyBinder {
	fn bind<'t>(self, t: &'t T) -> T where 'a: 't {
		*t
	}
}

impl<'a, T, U: Clone> Ref<'a, T, UnboundRef<U>> {
	pub fn cloned(self) -> Ref<'a, T, UnboundOwned<U>> {
		self.map(CloneBinder)
	}
}

impl<'a, T, U: Copy> Ref<'a, T, UnboundRef<U>> {
	pub fn copied(self) -> Ref<'a, T, UnboundOwned<U>> {
		self.map(CopyBinder)
	}
}

impl<'a, T, U> Deref for Ref<'a, T, UnboundRef<U>> {
	type Target = U;

	fn deref(&self) -> &Self::Target {
		&self.u
	}
}

impl<'a, T, U: UnboundIterator> Iterator for Ref<'a, T, U> {
	type Item = Ref<'a, T, U::UnboundItem>;

	fn next(&mut self) -> Option<Self::Item> {
		self.u.next().map(|item| {
			Ref {
				t: self.t.clone(),
				u: item
			}
		})
	}
}

impl<'a, C: 'a, T, U: UnboundContextualIterator<C>> ContextualIterator<C> for Ref<'a, T, U>
where
	<U as Unbound>::Bound<'a>: ContextualIterator<C, Item = <U::UnboundItem as Unbound>::Bound<'a>>
{
	type Item = Ref<'a, T, U::UnboundItem>;

	fn next_with(&mut self, context: &mut C) -> Option<Self::Item> {
		self.u.next_with(context).map(|item| {
			Ref {
				t: self.t.clone(),
				u: item
			}
		})
	}
}