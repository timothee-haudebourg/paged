use std::cmp::Ordering;

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

	pub fn push(&mut self, entry: T) {
		self.entries.push(entry)
	}

	pub fn binary_search_by_key<K: Ord>(
		&self,
		key: &K,
		f: impl Fn(&T) -> &K,
	) -> Result<u32, Ordering> {
		if self.entries.is_empty() {
			Err(Ordering::Equal)
		} else if f(self.entries.first().unwrap()) > key {
			Err(Ordering::Greater)
		} else if f(self.entries.last().unwrap()) < key {
			Err(Ordering::Less)
		} else {
			match self.entries.binary_search_by(|t| f(t).cmp(key)) {
				Ok(i) => Ok(i as u32),
				Err(_) => Err(Ordering::Equal),
			}
		}
	}
}

impl<T> sharded_slab::Clear for Page<T> {
	fn clear(&mut self) {
		self.entries.clear()
	}
}
