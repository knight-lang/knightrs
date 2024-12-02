use crate::container::RefCount;
use crate::parser::{ParseError, ParseErrorKind, Parseable, Parser};
use crate::program::{Compilable, Compiler};
use crate::strings::{StringError, StringSlice};
use crate::value::{Boolean, Integer, List, NamedType, ToBoolean, ToInteger, ToList};
use crate::{Environment, Options};
use std::borrow::Borrow;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)] // TODO, debug
pub struct KString(RefCount<StringSlice>);

pub trait ToKString {
	fn to_kstring(&self, env: &mut Environment) -> crate::Result<KString>;
}

impl NamedType for KString {
	#[inline]
	fn type_name(&self) -> &'static str {
		"String"
	}
}

impl Default for KString {
	fn default() -> Self {
		Self::from_slice(Default::default())
	}
}

impl From<&StringSlice> for KString {
	fn from(slice: &StringSlice) -> Self {
		Self::from_slice(slice)
	}
}

impl KString {
	pub fn from_slice(slice: &StringSlice) -> Self {
		let refcounted = RefCount::<str>::from(slice.as_str());
		// SAFETY: tood, but it is valid i think lol
		Self(unsafe { RefCount::from_raw(RefCount::into_raw(refcounted) as *const StringSlice) })
	}

	pub fn from_string_unchecked(source: String) -> Self {
		let refcounted = RefCount::<str>::from(source);
		// SAFETY: tood, but it is valid i think lol
		Self(unsafe { RefCount::from_raw(RefCount::into_raw(refcounted) as *const StringSlice) })
	}

	pub fn new_unvalidated(source: &str) -> Self {
		Self::from_slice(StringSlice::new_unvalidated(source))
	}

	#[cfg_attr(not(feature = "compliance"), inline)]
	pub fn new(
		source: impl AsRef<str>,
		opts: &Options,
	) -> Result<Self, crate::strings::StringError> {
		StringSlice::new(source, opts).map(Self::from_slice)
	}
}

impl Borrow<StringSlice> for KString {
	fn borrow(&self) -> &StringSlice {
		&self
	}
}

impl std::ops::Deref for KString {
	type Target = StringSlice;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl AsRef<StringSlice> for KString {
	fn as_ref(&self) -> &StringSlice {
		&self
	}
}

impl ToBoolean for KString {
	fn to_boolean(&self, _: &mut Environment) -> crate::Result<Boolean> {
		Ok(!self.is_empty())
	}
}

impl ToKString for KString {
	fn to_kstring(&self, _: &mut Environment) -> crate::Result<KString> {
		Ok(self.clone())
	}
}

impl ToInteger for KString {
	fn to_integer(&self, env: &mut Environment) -> crate::Result<Integer> {
		todo!()
	}
}

impl ToList for KString {
	fn to_list(&self, env: &mut Environment) -> crate::Result<List> {
		todo!()
	}
}

impl KString {
	/// Concatenates two strings together
	pub fn concat(&self, rhs: &StringSlice, opts: &Options) -> Result<Self, StringError> {
		if self.is_empty() {
			return Ok(rhs.to_owned());
		}

		if rhs.is_empty() {
			return Ok(self.clone());
		}

		Self::new(self.as_str().to_owned() + rhs.as_str(), opts)
	}

	pub fn remove_substr(&self, substr: &StringSlice) -> Self {
		let _ = substr;
		todo!();
	}
}

impl Parseable for KString {
	type Output = Self;

	fn parse(parser: &mut Parser<'_, '_>) -> Result<Option<Self::Output>, ParseError> {
		#[cfg(feature = "extensions")]
		if parser.opts().extensions.syntax.string_interpolation && parser.advance_if('`').is_some() {
			todo!();
		}

		let Some(quote) = parser.advance_if(|c| c == '\'' || c == '\"') else {
			return Ok(None);
		};

		let start = parser.location();

		// empty stings are allowed to exist
		let contents = parser.take_while(|c| c != quote).unwrap_or_default();

		if parser.advance_if(quote).is_none() {
			return Err(start.error(ParseErrorKind::MissingEndingQuote(quote)));
		}

		let string = KString::new(contents, parser.opts()).map_err(|err| start.error(err.into()))?;
		Ok(Some(string))
	}
}

unsafe impl Compilable for KString {
	fn compile(self, compiler: &mut Compiler, _: &Options) -> Result<(), ParseError> {
		compiler.push_constant(self.into());
		Ok(())
	}
}