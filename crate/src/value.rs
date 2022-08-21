// use crate::{Ast, Boolean, Environment, Error, Function, Integer, Result, SharedStr, Variable};
use crate::env::Environment;
use crate::{Ast, Error, Integer, Result, SharedStr, Variable};
use std::fmt::{self, Debug, Formatter};

/// A Value within Knight.
#[derive(Clone, PartialEq)]
pub enum Value {
	/// Represents the `NULL` value.
	Null,

	/// Represents the `TRUE` and `FALSE` values.
	Boolean(bool),

	/// Represents integers.
	Integer(Integer),

	/// Represents a string.
	SharedStr(SharedStr),

	/// Represents a variable.
	Variable(Variable),

	/// Represents a block of code.
	Ast(Ast),

	#[cfg(feature = "arrays")]
	Array(crate::Array),
}
#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Value: Send, Sync);

impl Default for Value {
	#[inline]
	fn default() -> Self {
		Self::Null
	}
}

impl Debug for Value {
	// note we need the custom impl becuase `Null()` and `Identifier(...)` are needed by the tester.
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match self {
			Self::Null => write!(f, "Null()"),
			Self::Boolean(boolean) => write!(f, "Boolean({boolean})"),
			Self::Integer(number) => write!(f, "Integer({number})"),
			Self::SharedStr(text) => write!(f, "Text({text})"),
			Self::Variable(variable) => write!(f, "Identifier({})", variable.name()),
			Self::Ast(ast) => write!(f, "{ast:?}"),
			#[cfg(feature = "arrays")]
			Self::Array(ary) => Debug::fmt(&ary, f),
		}
	}
}

impl From<()> for Value {
	#[inline]
	fn from(_: ()) -> Self {
		Self::Null
	}
}

impl From<bool> for Value {
	#[inline]
	fn from(boolean: bool) -> Self {
		Self::Boolean(boolean)
	}
}

impl From<Integer> for Value {
	#[inline]
	fn from(number: Integer) -> Self {
		Self::Integer(number)
	}
}

impl From<SharedStr> for Value {
	#[inline]
	fn from(text: SharedStr) -> Self {
		Self::SharedStr(text)
	}
}

impl From<Variable> for Value {
	#[inline]
	fn from(variable: Variable) -> Self {
		Self::Variable(variable)
	}
}

impl From<Ast> for Value {
	#[inline]
	fn from(inp: Ast) -> Self {
		Self::Ast(inp)
	}
}

#[cfg(feature = "arrays")]
impl From<crate::Array> for Value {
	#[inline]
	fn from(array: crate::Array) -> Self {
		Self::Array(array)
	}
}

pub trait Context: Sized {
	fn convert(value: &Value) -> Result<Self>;
}

impl Context for bool {
	fn convert(value: &Value) -> Result<Self> {
		match *value {
			Value::Null => Ok(false),
			Value::Boolean(boolean) => Ok(boolean),
			Value::Integer(number) => Ok(number != 0),
			Value::SharedStr(ref text) => Ok(!text.is_empty()),
			#[cfg(feature = "arrays")]
			Value::Array(ref ary) => Ok(!ary.is_empty()),
			_ => Err(Error::NoConversion { to: "Boolean", from: value.typename() }),
		}
	}
}

impl Context for Integer {
	fn convert(value: &Value) -> Result<Self> {
		match *value {
			Value::Null => Ok(0),
			Value::Boolean(boolean) => Ok(boolean as Self),
			Value::Integer(number) => Ok(number),
			Value::SharedStr(ref text) => text.to_integer(),
			#[cfg(feature = "arrays")]
			Value::Array(ref ary) => Ok(ary.len() as Self),
			_ => Err(Error::NoConversion { to: "Integer", from: value.typename() }),
		}
	}
}

impl Context for SharedStr {
	fn convert(value: &Value) -> Result<Self> {
		match *value {
			Value::Null => Ok(SharedStr::new("null").unwrap()),
			Value::Boolean(boolean) => Ok(SharedStr::new(boolean).unwrap()),
			Value::Integer(number) => Ok(SharedStr::new(number).unwrap()),
			Value::SharedStr(ref text) => Ok(text.clone()),
			#[cfg(feature = "arrays")]
			Value::Array(ref ary) => Ok(ary.to_knstr()),
			_ => Err(Error::NoConversion { to: "String", from: value.typename() }),
		}
	}
}

#[cfg(feature = "arrays")]
impl Context for crate::Array {
	fn convert(value: &Value) -> Result<Self> {
		match *value {
			Value::Null => Ok(Self::default()),
			Value::Boolean(boolean) => todo!(),
			Value::Integer(mut number) => {
				if number == 0 {
					return Ok(vec![0.into()].into());
				}

				// TODO: when log10 is finalized, add it in.
				let mut ary = Vec::new();

				let is_negative = if number < 0 {
					number = -number; // TODO: checked negation.
					true
				} else {
					false
				};

				while number != 0 {
					ary.push(Value::from(number % 10));
					number /= 10;
				}

				if is_negative {
					ary.push((-1).into());
				}

				ary.reverse();

				Ok(ary.into())
			}
			Value::SharedStr(ref text) => Ok(text
				.chars()
				.map(|c| Value::from(SharedStr::try_from(c.to_string()).unwrap()))
				.collect()),
			Value::Array(ref ary) => Ok(ary.clone()),
			_ => Err(Error::NoConversion { to: "Array", from: value.typename() }),
		}
	}
}

impl Value {
	/// Fetch the type's name.
	#[must_use = "getting the type name by itself does nothing."]
	pub const fn typename(&self) -> &'static str {
		match self {
			Self::Null => "Null",
			Self::Boolean(_) => "Boolean",
			Self::Integer(_) => "Integer",
			Self::SharedStr(_) => "SharedStr",
			Self::Variable(_) => "Variable",
			Self::Ast(_) => "Ast",
			#[cfg(feature = "arrays")]
			Self::Array(_) => "Array",
		}
	}

	/// Converts `self` to a [`bool`] according to the Knight spec.
	pub fn to_bool(&self) -> Result<bool> {
		Context::convert(self)
	}

	/// Converts `self` to an [`Integer`] according to the Knight spec.
	pub fn to_integer(&self) -> Result<Integer> {
		Context::convert(self)
	}

	/// Converts `self` to a [`SharedStr`] according to the Knight spec.
	pub fn to_knstr(&self) -> Result<SharedStr> {
		Context::convert(self)
	}

	#[cfg(feature = "arrays")]
	pub fn to_array(&self) -> Result<crate::Array> {
		Context::convert(self)
	}

	/// Executes the value.
	pub fn run(&self, env: &mut Environment) -> Result<Self> {
		match self {
			Self::Variable(variable) => {
				variable.fetch().ok_or_else(|| Error::UndefinedVariable(variable.name().clone()))
			}
			Self::Ast(ast) => ast.run(env),
			_ => Ok(self.clone()),
		}
	}
}
