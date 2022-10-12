use crate::env::Flags;
use crate::parse::{self, Parsable, Parser};
use crate::value::text::Encoding;
use crate::value::{integer::IntType, Runnable, Text, TextSlice, Value};
use crate::{Environment, Error, Mutable, RefCount, Result};
use std::borrow::Borrow;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

/// Represents a variable within Knight.
///
/// You'll never create variables directly; Instead, use [`Environment::lookup`].
#[derive_where(Clone)]
pub struct Variable<I, E>(RefCount<Inner<I, E>>);

struct Inner<I, E> {
	name: Text<E>,
	value: Mutable<Option<Value<I, E>>>,
}

#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Variable< (), ()>: Send, Sync);

impl<I: Debug, E> Debug for Variable<I, E> {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		if f.alternate() {
			f.debug_struct("Variable")
				.field("name", &self.name())
				.field("value", &self.0.value)
				.finish()
		} else {
			write!(f, "Variable({})", self.name())
		}
	}
}

impl<I, E> Eq for Variable<I, E> {}
impl<I, E> PartialEq for Variable<I, E> {
	/// Checks to see if two variables are equal.
	///
	/// This checks to see if the two variables are pointing to the _exact same object_.
	fn eq(&self, rhs: &Self) -> bool {
		RefCount::ptr_eq(&self.0, &rhs.0)
	}
}

impl<I, E> Borrow<TextSlice<E>> for Variable<I, E> {
	/// Borrows the [`name`](Variable::name) of the variable.
	fn borrow(&self) -> &TextSlice<E> {
		self.name()
	}
}

impl<I, E> Hash for Variable<I, E> {
	/// Hashes the [`name`](Variable::name) of the variable.
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name().hash(state);
	}
}

impl<I, E> crate::value::NamedType for Variable<I, E> {
	const TYPENAME: &'static str = "Variable";
}

/// Indicates that a a variable name was illegal.
///
/// While the enum itself is not feature gated, every one of its variants requires `compliance` to
/// be enabled. This means that if `compliance` isn't enabled, then it's impossible to ever
/// construct this type.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IllegalVariableName {
	/// The name was empty.
	#[cfg(feature = "compliance")]
	#[cfg_attr(docsrs, doc(cfg(feature = "compliance")))]
	Empty,

	/// The name was too long.
	#[cfg(feature = "compliance")]
	#[cfg_attr(docsrs, doc(cfg(feature = "compliance")))]
	TooLong(usize),

	/// The name had an illegal character at the beginning.
	#[cfg(feature = "compliance")]
	#[cfg_attr(docsrs, doc(cfg(feature = "compliance")))]
	IllegalStartingChar(char),

	/// The name had an illegal character in the middle.
	#[cfg(feature = "compliance")]
	#[cfg_attr(docsrs, doc(cfg(feature = "compliance")))]
	IllegalBodyChar(char),
}

impl std::error::Error for IllegalVariableName {}

impl Display for IllegalVariableName {
	fn fmt(&self, #[allow(unused)] f: &mut Formatter) -> fmt::Result {
		match *self {
			#[cfg(feature = "compliance")]
			Self::Empty => write!(f, "empty variable name supplied"),

			#[cfg(feature = "compliance")]
			Self::TooLong(count) => {
				write!(f, "variable name was too long ({count} > {})", Variable::<(), ()>::MAX_NAME_LEN)
			}

			#[cfg(feature = "compliance")]
			Self::IllegalStartingChar(chr) => write!(f, "variable names cannot start with {chr:?}"),

			#[cfg(feature = "compliance")]
			Self::IllegalBodyChar(chr) => write!(f, "variable names cannot include with {chr:?}"),
		}
	}
}

impl<I, E> Variable<I, E> {
	/// Maximum length a name can have when [`verify_variable_names`](
	/// crate::env::flags::ComplianceFlags::verify_variable_names) is enabled.
	pub const MAX_NAME_LEN: usize = 127;

	#[cfg(feature = "compliance")]
	fn validate_name(name: &TextSlice<E>) -> std::result::Result<(), IllegalVariableName>
	where
		E: Encoding,
	{
		if Self::MAX_NAME_LEN < name.len() {
			return Err(IllegalVariableName::TooLong(name.len()));
		}

		let first = name.chars().next().ok_or(IllegalVariableName::Empty)?;
		if !first.is_lower() {
			return Err(IllegalVariableName::IllegalStartingChar(first.inner()));
		}

		if let Some(bad) = name.chars().find(|&c| !c.is_lower() && !c.is_numeric()) {
			return Err(IllegalVariableName::IllegalBodyChar(bad.inner()));
		}

		Ok(())
	}

	pub(crate) fn new(name: Text<E>, flags: &Flags) -> std::result::Result<Self, IllegalVariableName>
	where
		E: Encoding,
	{
		#[cfg(feature = "compliance")]
		if flags.compliance.verify_variable_names {
			Self::validate_name(&name)?;
		}

		let _ = flags;
		Ok(Self(Inner { name, value: None.into() }.into()))
	}

	/// Fetches the name of the variable.
	#[must_use]
	pub fn name(&self) -> &Text<E> {
		&self.0.name
	}

	/// Assigns a new value to the variable, returning whatever the previous value was.
	pub fn assign(&self, new: Value<I, E>) -> Option<Value<I, E>> {
		(self.0).value.write().replace(new)
	}

	/// Fetches the last value assigned to `self`, returning `None` if it haven't been assigned yet.
	#[must_use]
	pub fn fetch(&self) -> Option<Value<I, E>>
	where
		I: Clone,
	{
		(self.0).value.read().clone()
	}
}

impl<I: Clone, E> Runnable<I, E> for Variable<I, E> {
	fn run(&self, _env: &mut Environment<I, E>) -> Result<Value<I, E>> {
		self.fetch().ok_or_else(|| Error::UndefinedVariable(self.name().to_string()))
	}
}

impl<I: IntType, E: crate::value::text::Encoding> Parsable<I, E> for Variable<I, E> {
	type Output = Self;

	fn parse(parser: &mut Parser<'_, '_, I, E>) -> parse::Result<Option<Self>> {
		let Some(identifier) = parser.take_while(|chr| chr.is_lower() || chr.is_numeric()) else {
			return Ok(None);
		};

		match parser.env().lookup(identifier) {
			Ok(value) => Ok(Some(value)),
			Err(err) => match err {
				// When there's no compliance issues, there'll be nothing to match.
				#[cfg(feature = "compliance")]
				err => Err(parser.error(parse::ErrorKind::IllegalVariableName(err))),
			},
		}
	}
}
