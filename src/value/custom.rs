use crate::containers::{MaybeSendSync, RefCount};
use crate::value::integer::IntType;
use crate::value::text::Encoding;
use crate::value::{
	Boolean, Integer, List, NamedType, Runnable, Text, ToBoolean, ToInteger, ToList, ToText,
};
use crate::{Environment, Error, Result, Value};
use std::cmp::Ordering;
use std::fmt::Display;
use std::hash::{Hash, Hasher};

/// A type that can hold custom data that's not a part of vanilla Knight.
///
/// This is a simple wrapper around a [`Refcount`] of [`CustomType`]. All the meat is within
/// [`CustomType`].
#[derive_where(Debug, Clone)]
pub struct Custom<I, E>(RefCount<dyn CustomType<I, E>>);

impl<I: Eq, E> Eq for Custom<I, E> {}
impl<I: PartialEq, E> PartialEq for Custom<I, E> {
	fn eq(&self, rhs: &Self) -> bool {
		RefCount::ptr_eq(&self.0, &rhs.0)
	}
}

impl<I: Hash, E> Hash for Custom<I, E> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		(RefCount::as_ptr(&self.0) as *const u8 as usize).hash(state);
	}
}

impl<I, E, T: CustomType<I, E> + 'static> From<RefCount<T>> for Custom<I, E> {
	fn from(inp: RefCount<T>) -> Self {
		Self(inp as _)
	}
}

impl<I, E> Custom<I, E> {
	/// A helper method to create a [`Custom`].
	pub fn new<T: CustomType<I, E> + 'static>(data: T) -> Self {
		Self(RefCount::from(data) as _)
	}
}

// // #[derive(Debug)]
// // pub struct Map< I, E>(std::collections::HashMap< I, E>);
// // impl< I, E> CustomType< I, E> for Foo {
// // 	fn to_custom(self: RefCount<Self>) -> Custom< I, E> {
// // 		self.into()
// // 	}
// }

/// Trait for custom types.
///
/// The only required function is [`CustomType::to_custom`] (see below for details). Every other
/// function is supplied with a sane default (generally just returning an [`Error::NoConversion`]/
/// [`Error::TypeError`]), but can be overridden to provide actual implementations.
///
/// # `to_custom`
/// Due to limitations in the Rust's type system, there's no way to convert from an `Arc<MyType>` to
/// an `Arc<dyn CustomType>`. As such, it's required for implementations to supply this, however
/// it's literally as simple as calling `self.into()`.
///
/// # Examples
/// ```
/// <todo
///
#[allow(unused_variables)]
pub trait CustomType<I, E>: std::fmt::Debug + MaybeSendSync {
	fn to_custom(self: RefCount<Self>) -> Custom<I, E>;

	fn typename(&self) -> &'static str {
		std::any::type_name::<Self>()
	}

	fn run(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Value<I, E>> {
		Ok(self.to_custom().into())
	}

	fn to_text(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Text<E>>
	where
		E: Encoding,
		I: Display,
	{
		Err(Error::NoConversion { to: Text::<E>::TYPENAME, from: self.typename() })
	}

	fn to_integer(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Integer<I>>
	where
		I: IntType,
	{
		Err(Error::NoConversion { to: Integer::<I>::TYPENAME, from: self.typename() })
	}

	fn to_boolean(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Boolean>
	where
		I: IntType,
	{
		Err(Error::NoConversion { to: Boolean::TYPENAME, from: self.typename() })
	}

	fn to_list(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<List<I, E>>
	where
		I: IntType,
	{
		Err(Error::NoConversion { to: List::<I, E>::TYPENAME, from: self.typename() })
	}

	fn head(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "["))
	}

	fn tail(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "]"))
	}

	fn length(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Integer::<I>::try_from(self.to_list(env)?.len()).map(Value::from)
	}

	fn ascii(self: RefCount<Self>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "ASCII"))
	}

	fn add(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "+"))
	}

	fn subtract(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "-"))
	}

	fn multiply(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "*"))
	}

	fn divide(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "/"))
	}

	fn remainder(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "%"))
	}

	fn power(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "^"))
	}

	fn compare(
		self: RefCount<Self>,
		rhs: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Ordering>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "<cmp>"))
	}

	fn assign(self: RefCount<Self>, rhs: Value<I, E>, env: &mut Environment<I, E>) -> Result<()>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "="))
	}

	fn get(
		self: RefCount<Self>,
		start: usize,
		len: usize,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "GET"))
	}

	fn set(
		self: RefCount<Self>,
		start: usize,
		len: usize,
		replacement: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		Err(Error::TypeError(self.typename(), "GET"))
	}
}

impl<I: Display, E: Encoding> ToText<I, E> for Custom<I, E> {
	fn to_text(&self, env: &mut Environment<I, E>) -> Result<Text<E>> {
		self.0.clone().to_text(env)
	}
}

impl<I: IntType, E> ToInteger<I, E> for Custom<I, E> {
	fn to_integer(&self, env: &mut Environment<I, E>) -> Result<Integer<I>> {
		self.0.clone().to_integer(env)
	}
}

impl<I: IntType, E> ToBoolean<I, E> for Custom<I, E> {
	fn to_boolean(&self, env: &mut Environment<I, E>) -> Result<Boolean> {
		self.0.clone().to_boolean(env)
	}
}

impl<I: IntType, E> ToList<I, E> for Custom<I, E> {
	fn to_list(&self, env: &mut Environment<I, E>) -> Result<List<I, E>> {
		self.0.clone().to_list(env)
	}
}

impl<I, E> Runnable<I, E> for Custom<I, E> {
	fn run(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>> {
		self.0.clone().run(env)
	}
}

impl<I, E> Custom<I, E> {
	pub fn typename(&self) -> &'static str {
		self.0.typename()
	}

	pub fn run(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>> {
		self.0.clone().run(env)
	}

	pub fn head(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().head(env)
	}

	pub fn tail(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().tail(env)
	}

	pub fn length(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().length(env)
	}

	pub fn ascii(&self, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().ascii(env)
	}

	pub fn add(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().add(rhs, env)
	}

	pub fn subtract(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().subtract(rhs, env)
	}

	pub fn multiply(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().multiply(rhs, env)
	}

	pub fn divide(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().divide(rhs, env)
	}

	pub fn remainder(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().remainder(rhs, env)
	}

	pub fn power(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().power(rhs, env)
	}

	pub fn compare(&self, rhs: &Value<I, E>, env: &mut Environment<I, E>) -> Result<Ordering>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().compare(rhs, env)
	}

	pub fn assign(&self, rhs: Value<I, E>, env: &mut Environment<I, E>) -> Result<()>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().assign(rhs, env)
	}

	pub fn get(&self, start: usize, len: usize, env: &mut Environment<I, E>) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().get(start, len, env)
	}

	pub fn set(
		&self,
		start: usize,
		len: usize,
		replacement: &Value<I, E>,
		env: &mut Environment<I, E>,
	) -> Result<Value<I, E>>
	where
		I: IntType,
		E: Encoding,
	{
		self.0.clone().set(start, len, replacement, env)
	}
}
