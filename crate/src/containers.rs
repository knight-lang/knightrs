use std::ops::{Deref, DerefMut};

#[derive(Debug, PartialEq, Eq)]
pub struct RefCount<T>(
	#[cfg(feature = "multithreaded")] std::sync::Arc<T>,
	#[cfg(not(feature = "multithreaded"))] std::rc::Rc<T>,
);

impl<T> Clone for RefCount<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T> From<T> for RefCount<T> {
	#[inline]
	fn from(inp: T) -> Self {
		Self(inp.into())
	}
}

impl<T> Deref for RefCount<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

#[derive(Debug)]
pub struct Mutable<T>(
	#[cfg(feature = "multithreaded")] std::sync::RwLock<T>,
	#[cfg(not(feature = "multithreaded"))] std::cell::RefCell<T>,
);

impl<T> From<T> for Mutable<T> {
	#[inline]
	fn from(inp: T) -> Self {
		Self(inp.into())
	}
}

impl<T> Mutable<T> {
	#[inline]
	pub fn read(&self) -> impl Deref<Target = T> + '_ {
		#[cfg(feature = "multithreaded")]
		{
			self.0.read().unwrap()
		}

		#[cfg(not(feature = "multithreaded"))]
		{
			self.0.borrow()
		}
	}

	#[inline]
	pub fn write(&self) -> impl DerefMut<Target = T> + '_ {
		#[cfg(feature = "multithreaded")]
		{
			self.0.write().unwrap()
		}

		#[cfg(not(feature = "multithreaded"))]
		{
			self.0.borrow_mut()
		}
	}
}
