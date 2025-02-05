use crate::parser::VariableName;

// TODO: make this just runtime error and parse error?
#[derive(Error, Debug)]
pub enum Error {
	#[error("{0}")]
	Todo(String),

	#[error("{0}")]
	Stacktrace(String),

	#[error("{0}")]
	StringError(#[from] crate::strings::StringError),

	#[error("{0}")]
	IntegerError(#[from] crate::value::integer::IntegerError),

	#[error("{0}")]
	ParseError(String),

	#[error("undefined variable {0} accessed")]
	UndefinedVariable(VariableName<'static>),

	#[error("bad type {type_name} to function {function:?}")]
	TypeError { type_name: &'static str, function: &'static str },

	/// Indicates that either `GET` or `SET` were given an index that was out of bounds.
	#[error("end index {index} is out of bounds for length {len}")]
	IndexOutOfBounds { len: usize, index: usize },

	#[error("list is too large")]
	ListIsTooLarge,

	#[error("(quit with exit status {0})")]
	// #[cfg(any(doc, feature = "embedded"))]
	#[cfg(feature = "embedded")]
	Exit(i32),

	#[error("Conversion to {to} not defined for {from}")]
	ConversionNotDefined { to: &'static str, from: &'static str },

	#[error("I/O error happened during {func}: {err}")]
	IoError { func: &'static str, err: std::io::Error },

	/// The types to a function were correct, but their values weren't somehow.
	#[error("domain error: {0}")]
	DomainError(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<crate::parser::ParseError<'_>> for Error {
	fn from(err: crate::parser::ParseError<'_>) -> Self {
		Self::ParseError(err.to_string())
	}
}
