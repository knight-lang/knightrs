//! Types relating to the execution of Knight code.
//!
//! See [`Environment`] for more details.

use crate::{RuntimeError, RcString, Value, Function};
use std::collections::{HashSet, HashMap};
use std::fmt::{self, Debug, Formatter};
use std::io::{self, Write, Read};

mod builder;
mod variable;

pub use builder::Builder;
pub use variable::Variable;

type RunCommand = dyn FnMut(&str) -> Result<RcString, RuntimeError>;

/// The execution environment for Knight programs.
///
/// To aid in embedding Knight in other systems, the [`Environment`] provides complete control over the stdin, stdout,
/// and output of the ["system" (`` ` ``)](crate::function::system), in addition to keeping track of all relevant
/// variables. Because of this, the environment must always be passed when calling [`Value::run`](crate::Value::run).
///
/// This is in contrast with most other Knight implementations, which usually have a singular, global "environment", and
///
/// # Examples
/// ```rust,no_run
/// # use knightrs::Environment;
/// # use std::io::{Read, Write};
/// let mut env = Environment::new();
///
/// // Write to stdout.
/// writeln!(env, "Hello, world!");
///
/// // Read from stdin.
/// let mut str = String::new();
/// env.read_to_string(&mut str).expect("cant read from stdin!");
///
/// // execute command
/// println!("The stdout of `ls -al` is {}", env.run_command("ls -al").expect("`ls -al` failed"));
///
/// // create a variable
/// let var = env.get("foobar");
/// assert_eq!(var, env.get("foobar")); // both variables are the same.
/// ```
pub struct Environment<I, O> {
	// We use a `HashSet` because we want the variable to own its name, which a `HashMap` wouldn't allow for. (or would
	// have redundant allocations.)
	vars: HashSet<Variable<I, O>>,
	stdin: I,
	stdout: O,
	run_command: Box<RunCommand>,
	functions: HashMap<char, Function<I, O>>
}

impl<I, O> Debug for Environment<I, O> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("Environment")
			.field("nvars", &self.vars.len())
			.finish()
	}
}

impl<I, O> Default for Environment<I, O> {
	fn default() -> Self {
		Self::new()
	}
}

impl<I, O> Environment<I, O> {
	/// Creates an empty [`Environment`].
	///
	/// # Examples
	/// ```rust
	/// # use knightrs::Environment;
	/// let env = Environment::new();
	///
	/// // ...do stuff with `env`.
	/// ```
	#[must_use = "simply creating an environment doesn't do anything."]
	pub fn new() -> Self {
		Self::builder().build()
	}

	/// Creates a new [`Builder`].
	///
	/// This is simply a helper function, and is provided so that you don't have to import [`Builder`].
	///
	/// # Examples
	/// ```rust
	/// # use knightrs::Environment;
	/// let env = Environment::builder().disable_system().build();
	///
	/// // ... do stuff with `env`.
	/// ```
	#[must_use = "simply creating a builder does nothing."]
	pub fn builder() -> Builder<I, O> {
		Builder::default()
	}

	/// Retrieves the variable with the given name.
	///
	/// This method will always succeed; if this is the first time that `name` has been seen by `self`, a new (unassigned
	/// ) variable will be created. 
	///
	/// # Examples
	/// ```rust
	/// # use knightrs::Environment;
	/// let mut env = Environment::new();
	/// let var = env.get("plato");
	///
	/// assert_eq!(var, env.get("plato"));
	/// ```
	pub fn get<N: AsRef<str> + ToString + ?Sized>(&mut self, name: &N) -> Variable<I, O> {
		if let Some(inner) = self.vars.get(name.as_ref()) {
			return inner.clone();
		}

		let variable = Variable::_new(name.to_string().into_boxed_str());

		self.vars.insert(variable.clone());

		variable
	}

	/// Executes `cmd` as a system command, returning the stdout of the child process.
	///
	/// This will internally call the value that was set for [`Builder::run_command`]. See that function for more details
	/// on, eg, the default value.
	///
	/// # Examples
	/// ```rust
	/// # use knightrs::Environment;
	/// let mut env = Environment::new();
	///
	/// assert_eq!(env.run_command("echo 'hello, knight!'").unwrap().as_str(), "hello, knight!\n");
	/// ```
	pub fn run_command(&mut self, cmd: &str) -> Result<RcString, RuntimeError> {
		(self.run_command)(cmd)
	}


	/// Gets the function associate dwith the given `name`, returning `None` if no such function exists.
	#[must_use = "fetching a function does nothing by itself"]
	pub fn get_function(&self, name: char) -> Option<Function<I, O>> {
		self.functions.get(&name).cloned()
	}

	/// Registers a new function with the given name, discarding any previous value associated with it.
	pub fn register_function(&mut self, name: char, arity: usize, func: crate::function::FuncPtr<I, O>) {
		self.functions.insert(name, Function::_new(name, arity, func));
	}

}

impl<I: Read, O> Read for Environment<I, O> {
	/// Read bytes into `data` from `self`'s `stdin`.
	///
	/// The `stdin` can be customized at creation via [`Builder::stdin`].
	#[inline]
	fn read(&mut self, data: &mut [u8]) -> io::Result<usize> {
		self.stdin.read(data)
	}
}

impl<I, O: Write> Write for Environment<I, O> {
	/// Writes `data`'s bytes into `self`'s `stdout`.
	///
	/// The `stdin` can be customized at creation via [`Builder::stdin`].
	#[inline]
	fn write(&mut self, data: &[u8]) -> io::Result<usize> {
		self.stdout.write(data)
	}

	#[inline]
	fn flush(&mut self) -> io::Result<()> {
		self.stdout.flush()
	}
}
