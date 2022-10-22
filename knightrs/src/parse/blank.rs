use super::*;

/// A [`Parsable`] that strips whitespace and comments.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Blank;

/// The never type's replacement.
pub enum Never {}

impl<I> From<Never> for Value<I> {
	fn from(never: Never) -> Self {
		match never {}
	}
}

impl<I> Parsable<I> for Blank {
	type Output = Never;

	fn parse(parser: &mut Parser<'_, '_, I>) -> Result<Option<Self::Output>> {
		if parser.strip_whitespace_and_comments().is_some() {
			Err(parser.error(ErrorKind::RestartParsing))
		} else {
			Ok(None)
		}
	}
}
