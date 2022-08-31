use crate::{Environment, Error, Integer, RefCount, Result, SharedText, Text, Value};
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::ops::{Range, RangeInclusive};

#[derive(Clone, Default)]
pub struct List(Option<RefCount<Inner>>);

enum Inner {
	Boxed(Value),
	IntRange(Range<Integer>),        // strictly increasing, nonempty
	CharRange(RangeInclusive<char>), // strictly increasing, nonempty
	Slice(Box<[Value]>),             // nonempty slice
	Cons(List, List),                // neither list is empty
	Repeat(List, usize),             // the usize is >= 2

	#[cfg(feature = "negative-ranges")]
	IntRangeRev(Range<Integer>), // strictly increasing, nonempty
	#[cfg(feature = "negative-ranges")]
	CharRangeRev(RangeInclusive<char>), // strictly increasing, nonempty.
}

impl PartialEq for List {
	fn eq(&self, rhs: &Self) -> bool {
		if std::ptr::eq(self, rhs) {
			return true;
		}

		if self.len() != rhs.len() {
			return false;
		}

		self.iter().zip(rhs.iter()).all(|(l, r)| l == r)
	}
}

impl Debug for List {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		f.debug_list().entries(self.iter()).finish()
	}
}

impl From<Value> for List {
	fn from(value: Value) -> Self {
		Self::_new(Inner::Boxed(value))
	}
}

impl TryFrom<Range<Integer>> for List {
	type Error = Error;

	fn try_from(range: Range<Integer>) -> Result<Self> {
		match range.start.cmp(&range.end) {
			Ordering::Less => Ok(Self::_new(Inner::IntRange(range))),
			Ordering::Equal => Ok(Self(None)),

			#[cfg(feature = "negative-ranges")]
			Ordering::Greater => {
				Ok(Self::_new(Inner::IntRangeRev(Range { start: range.end, end: range.start })))
			}
			#[cfg(not(feature = "negative-ranges"))]
			_ => Err(Error::DomainError("start < end for list")),
		}
	}
}

impl TryFrom<RangeInclusive<char>> for List {
	type Error = Error;

	fn try_from(range: RangeInclusive<char>) -> Result<Self> {
		match range.start().cmp(&range.end()) {
			Ordering::Less => Ok(Self::_new(Inner::CharRange(range))),
			Ordering::Equal => Ok(Self(None)),

			#[cfg(feature = "negative-ranges")]
			Ordering::Greater => {
				Ok(Self::_new(Inner::CharRangeRev(RangeInclusive::new(*range.end(), *range.start()))))
			}
			#[cfg(not(feature = "negative-ranges"))]
			_ => Err(Error::DomainError("start < end for list")),
		}
	}
}

// impl From<(List, List)> for List {
// 	fn from(cons: (List, List)) -> Self {
// 		if cons.0.len() == 0 {
// 			cons.1
// 		} else if rhs.len() == 0 {
// 			cons.0.clone()
// 		} else {
// 			Self::_new(Inner::Cons(cons.0.clone(), rhs.clone()))
// 		}

// 		// cons.0.concat(&cons.1)
// 		let _ = cons;
// 		todo!();
// 	}
// }

impl From<Box<[Value]>> for List {
	fn from(list: Box<[Value]>) -> Self {
		match list.len() {
			0 => Self::default(),
			// OPTIMIZE: is there a way to not do `.clone()`?
			1 => list[0].clone().into(),
			_ => Self::_new(Inner::Slice(list)),
		}
	}
}

impl From<Vec<Value>> for List {
	fn from(list: Vec<Value>) -> Self {
		list.into_boxed_slice().into()
	}
}

impl FromIterator<Value> for List {
	fn from_iter<T: IntoIterator<Item = Value>>(iter: T) -> Self {
		iter.into_iter().collect::<Vec<Value>>().into()
	}
}

impl List {
	fn _new(inner: Inner) -> Self {
		Self(Some(inner.into()))
	}

	fn inner(&self) -> Option<&Inner> {
		self.0.as_deref()
	}

	pub fn is_empty(&self) -> bool {
		if self.0.is_none() {
			return true;
		}

		debug_assert_ne!(self.len(), 0);
		false
	}

	pub fn len(&self) -> usize {
		match self.inner() {
			None => 0,
			Some(Inner::Boxed(_)) => 1,
			Some(Inner::IntRange(rng)) => (rng.end - rng.start) as usize,
			Some(Inner::CharRange(rng)) => ((*rng.end() as u32) - (*rng.start() as u32) + 1) as usize, // todo: are these two right?
			Some(Inner::Slice(slice)) => slice.len(),
			Some(Inner::Cons(lhs, rhs)) => lhs.len() + rhs.len(),
			Some(Inner::Repeat(list, amount)) => list.len() * amount,

			#[cfg(feature = "negative-ranges")]
			Some(Inner::IntRangeRev(rng)) => (rng.end - rng.start) as usize,
			#[cfg(feature = "negative-ranges")]
			Some(Inner::CharRangeRev(rng)) => ((*rng.end() as u32) - (*rng.start() as u32) + 1) as usize, // todo: are these two right?
		}
	}

	pub fn to_text(&self) -> Result<SharedText> {
		const NEWLINE: &Text = unsafe { Text::new_unchecked("\n") };

		self.join(NEWLINE)
	}

	pub fn concat<'a>(&self, rhs: &List) -> Self {
		if self.len() == 0 {
			rhs.clone()
		} else if rhs.len() == 0 {
			self.clone()
		} else {
			Self::_new(Inner::Cons(self.clone(), rhs.clone()))
		}
	}

	pub fn repeat(&self, amount: usize) -> Self {
		match amount {
			0 => Self::default(),
			1 => self.clone(),
			_ => Self::_new(Inner::Repeat(self.clone(), amount)),
		}
	}

	pub fn join(&self, sep: &Text) -> Result<SharedText> {
		let mut joined = SharedText::builder();

		let mut is_first = true;
		for ele in self {
			if is_first {
				is_first = false;
			} else {
				joined.push(&sep);
			}

			joined.push(&ele.to_text()?);
		}

		Ok(joined.finish())
	}

	pub fn iter(&self) -> Iter<'_> {
		match self.inner() {
			None => Iter::Empty,
			Some(Inner::Boxed(val)) => Iter::Boxed(val),
			Some(Inner::IntRange(rng)) => Iter::IntRange(rng.clone()),
			Some(Inner::CharRange(rng)) => Iter::CharRange(rng.clone()),
			Some(Inner::Slice(slice)) => Iter::Slice(slice.iter()),
			Some(Inner::Cons(_, _)) => todo!(),
			Some(Inner::Repeat(list, amount)) => {
				Iter::Repeat(Box::new(list.iter()).cycle(), list.len() * *amount)
			}

			#[cfg(feature = "negative-ranges")]
			Some(Inner::IntRangeRev(rng)) => Iter::IntRangeRev(rng.clone()),
			#[cfg(feature = "negative-ranges")]
			Some(Inner::CharRangeRev(rng)) => Iter::CharRangeRev(rng.clone()),
		}
	}

	pub fn contains(&self, value: &Value) -> bool {
		match self.inner() {
			None => false,
			Some(Inner::Boxed(val)) => val == value,
			Some(Inner::IntRange(rng)) => {
				if let Value::Integer(integer) = value {
					rng.contains(integer)
				} else {
					false
				}
			}
			Some(Inner::CharRange(rng)) => {
				if let Value::SharedText(text) = value {
					text.into_iter().next().map_or(false, |c| rng.contains(&c))
				} else {
					false
				}
			}
			Some(Inner::Cons(lhs, rhs)) => lhs.contains(value) || rhs.contains(value),
			Some(Inner::Slice(slice)) => slice.contains(value),
			Some(Inner::Repeat(list, _)) => list.contains(value),

			#[cfg(feature = "negative-ranges")]
			Some(Inner::IntRangeRev(_rng)) => todo!(),
			#[cfg(feature = "negative-ranges")]
			Some(Inner::CharRangeRev(_rng)) => todo!(),
		}
	}

	#[cfg(feature = "list-extensions")]
	pub fn difference(&self, rhs: &Self) -> Self {
		let mut list = Vec::with_capacity(self.len() - rhs.len());

		for ele in self {
			if !rhs.contains(&ele) && !list.contains(&ele) {
				list.push(ele);
			}
		}

		list.into()
	}

	#[cfg(feature = "list-extensions")]
	pub fn map(&self, block: &Value, env: &mut Environment) -> Result<Self> {
		const UNDERSCORE: &'static Text = unsafe { Text::new_unchecked("_") };

		let arg = env.lookup(UNDERSCORE).unwrap();

		self
			.iter()
			.map(|ele| {
				arg.assign(ele);
				block.run(env)
			})
			.collect()
	}

	#[cfg(feature = "list-extensions")]
	pub fn reduce(&self, block: &Value, env: &mut Environment) -> Result<Option<Value>> {
		const ACCUMULATE: &'static Text = unsafe { Text::new_unchecked("a") };
		const UNDERSCORE: &'static Text = unsafe { Text::new_unchecked("_") };

		let mut iter = self.iter();

		let acc = env.lookup(ACCUMULATE).unwrap();
		if let Some(init) = iter.next() {
			acc.assign(init);
		} else {
			return Ok(None);
		}

		let arg = env.lookup(UNDERSCORE).unwrap();
		for ele in iter {
			arg.assign(ele);
			acc.assign(block.run(env)?);
		}

		Ok(Some(acc.fetch().unwrap()))
	}

	#[cfg(feature = "list-extensions")]
	pub fn filter(&self, block: &Value, env: &mut Environment) -> Result<Self> {
		const UNDERSCORE: &'static Text = unsafe { Text::new_unchecked("_") };

		let arg = env.lookup(UNDERSCORE).unwrap();

		self
			.iter()
			.filter_map(|ele| {
				arg.assign(ele.clone());

				block.run(env).and_then(|b| b.to_bool()).and(Ok(Some(ele))).transpose()
			})
			.collect()
	}
}

impl<'a> IntoIterator for &'a List {
	type Item = Value;
	type IntoIter = Iter<'a>;

	fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
		self.iter()
	}
}

#[derive(Clone)]
pub enum Iter<'a> {
	Empty,
	Boxed(&'a Value),
	IntRange(Range<Integer>),
	CharRange(RangeInclusive<char>),
	Cons(List, List),
	Slice(std::slice::Iter<'a, Value>),
	Repeat(std::iter::Cycle<Box<Self>>, usize),

	#[cfg(feature = "negative-ranges")]
	IntRangeRev(Range<Integer>),
	#[cfg(feature = "negative-ranges")]
	CharRangeRev(RangeInclusive<char>),
}

impl<'a> Iterator for Iter<'a> {
	type Item = Value;

	fn next(&mut self) -> Option<Self::Item> {
		match self {
			Self::Empty => None,
			Self::Boxed(value) => {
				let ret = Some(value.clone());
				*self = Self::Empty;
				ret
			}
			Self::IntRange(rng) => rng.next().map(Value::from),
			Self::CharRange(rng) => rng.next().map(|c| SharedText::new(c).unwrap().into()),
			Self::Slice(iter) => iter.next().cloned(),
			Self::Cons(_, _) => todo!(),

			Self::Repeat(_, 0) => None,
			Self::Repeat(iter, n) => {
				*n -= 1;
				let value = iter.next();
				debug_assert!(value.is_some());
				value
			}

			// TODO: fixme, this arent correct
			#[cfg(feature = "negative-ranges")]
			Self::IntRangeRev(rng) => rng.next().map(Value::from),
			#[cfg(feature = "negative-ranges")]
			Self::CharRangeRev(rng) => rng.next().map(|c| SharedText::new(c).unwrap().into()),
		}
	}
}