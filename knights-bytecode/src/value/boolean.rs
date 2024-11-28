use crate::value::{Integer, KString, List, ToInteger, ToKString, ToList};
use crate::{Environment, Result};

/// The boolean type within Knight.
pub type Boolean = bool;

/// Represents the ability to be converted to a [`Boolean`].
pub trait ToBoolean {
	/// Converts `self` to a [`Boolean`].
	fn to_boolean(&self, env: &mut Environment) -> Result<Boolean>;
}

impl ToBoolean for Boolean {
	/// Simply returns `self`.
	#[inline]
	fn to_boolean(&self, _: &mut Environment) -> Result<Self> {
		Ok(*self)
	}
}

impl ToInteger for Boolean {
	/// Returns `1` for true and `0` for false.
	#[inline]
	fn to_integer(&self, _: &mut Environment) -> Result<Integer> {
		if *self {
			Ok(Integer::ONE)
		} else {
			Ok(Integer::ZERO)
		}
	}
}

impl ToList for Boolean {
	/// Returns an empty list for `false`, and a list with just `self` if true.
	#[inline]
	fn to_list(&self, _: &mut Environment) -> Result<List> {
		if *self {
			Ok(List::boxed((*self).into()))
		} else {
			Ok(List::default())
		}
	}
}

impl ToKString for Boolean {
	/// Returns `"true"` for true and `"false"` for false.
	#[inline]
	fn to_kstring(&self, _: &mut Environment) -> Result<KString> {
		todo!()
		// const TRUE_TEXT: &TextSlice = unsafe { TextSlice::new_unchecked("true") };
		// const FALSE_TEXT: &TextSlice = unsafe { TextSlice::new_unchecked("false") };

		// if *self {
		// 	Ok(TRUE_TEXT.into())
		// } else {
		// 	Ok(FALSE_TEXT.into())
		// }
	}
}
