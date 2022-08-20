use crate::{Error, KnStr, Result, SharedStr, Value};
use std::collections::HashSet;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, Read, Write};

cfg_if::cfg_if! {
	if #[cfg(feature="multithreaded")] {
		type SystemCommand = dyn FnMut(&KnStr) -> Result<SharedStr> + Send + Sync;
		type Stdin = dyn Read + Send + Sync;
		type Stdout = dyn Write + Send + Sync;
	} else {
		type SystemCommand = dyn FnMut(&KnStr) -> Result<SharedStr>;
		type Stdin = dyn Read;
		type Stdout = dyn Write;
	}
}

pub struct Environment {
	// We use a `HashSet` because we want the variable to own its name, which a `HashMap`
	// wouldn't allow for. (or would have redundant allocations.)
	variables: HashSet<Variable>,
	stdin: BufReader<Box<Stdin>>,
	stdout: Box<Stdout>,
	system: Box<SystemCommand>,
}

#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Environment: Send, Sync);

impl Default for Environment {
	fn default() -> Self {
		Self {
			variables: HashSet::default(),
			stdin: BufReader::new(Box::new(std::io::stdin())),
			stdout: Box::new(std::io::stdout()),
			system: Box::new(|cmd: &KnStr| {
				use std::process::{Command, Stdio};

				let output = Command::new("/bin/sh")
					.arg("-c")
					.arg(&**cmd)
					.stdin(Stdio::inherit())
					.output()
					.map(|out| String::from_utf8_lossy(&out.stdout).into_owned())?;

				Ok(SharedStr::try_from(output)?)
			}),
		}
	}
}

impl Environment {
	/// Fetches the variable corresponding to `name` in the environment, creating one if it's the
	/// first time that name has been requested
	pub fn lookup(&mut self, name: &KnStr) -> Variable {
		// OPTIMIZE: This does a double lookup, which isnt spectacular.
		if let Some(var) = self.variables.get(name) {
			return var.clone();
		}

		let variable = Variable(((name.to_boxed().into(), None.into())).into());
		self.variables.insert(variable.clone());
		variable
	}

	pub fn run_command(&mut self, command: &KnStr) -> Result<SharedStr> {
		(self.system)(command)
	}

	// this is here in case we want to add seeding
	pub fn random(&mut self) -> crate::Integer {
		rand::random::<crate::Integer>().abs()
	}

	pub fn play(&mut self, source: &KnStr) -> Result<Value> {
		crate::Parser::new(source).parse(self)?.run(self)
	}
}

impl Read for Environment {
	fn read(&mut self, data: &mut [u8]) -> io::Result<usize> {
		self.stdin.read(data)
	}
}

impl BufRead for Environment {
	fn fill_buf(&mut self) -> io::Result<&[u8]> {
		self.stdin.fill_buf()
	}

	fn consume(&mut self, amnt: usize) {
		self.stdin.consume(amnt);
	}

	fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
		self.stdin.read_line(buf)
	}
}

impl Write for Environment {
	fn write(&mut self, data: &[u8]) -> io::Result<usize> {
		self.stdout.write(data)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.stdout.flush()
	}
}

#[derive(Clone)]
#[rustfmt::skip]
pub struct Variable(
	#[cfg(feature = "multithreaded")]
	std::sync::Arc<(SharedStr, std::sync::RwLock<Option<Value>>)>,
	#[cfg(not(feature = "multithreaded"))]
	std::rc::Rc<(SharedStr, std::cell::RefCell<Option<Value>>)>,
);

#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Variable: Send, Sync);

impl Debug for Variable {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		if f.alternate() {
			f.debug_struct("Variable")
				.field("name", &self.name())
				.field("value", &self.fetch())
				.finish()
		} else {
			write!(f, "Variable({})", self.name())
		}
	}
}

impl std::borrow::Borrow<KnStr> for Variable {
	#[inline]
	fn borrow(&self) -> &KnStr {
		self.name()
	}
}

impl Eq for Variable {}
impl PartialEq for Variable {
	/// Checks to see if two variables are equal.
	///
	/// This'll just check to see if their names are equivalent. Techincally, this means that
	/// two variables with the same name, but derived from different [`Environment`]s will end up
	/// being the same
	#[inline]
	fn eq(&self, rhs: &Self) -> bool {
		self.name() == rhs.name()
	}
}

impl Hash for Variable {
	#[inline]
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.name().hash(state);
	}
}

impl Variable {
	/// Fetches the name of the variable.
	#[must_use]
	#[inline]
	pub fn name(&self) -> &SharedStr {
		&(self.0).0
	}

	/// Assigns a new value to the variable, returning whatever the previous value was.
	pub fn assign(&self, new: Value) -> Option<Value> {
		#[cfg(feature = "multithreaded")]
		{
			(self.0).1.write().expect("rwlock poisoned").replace(new)
		}

		#[cfg(not(feature = "multithreaded"))]
		{
			(self.0).1.replace(Some(new))
		}
	}

	/// Fetches the last value assigned to `self`, returning `None` if we haven't been assigned to yet.
	#[must_use]
	pub fn fetch(&self) -> Option<Value> {
		#[cfg(feature = "multithreaded")]
		{
			(self.0).1.read().expect("rwlock poisoned").clone()
		}

		#[cfg(not(feature = "multithreaded"))]
		{
			(self.0).1.borrow().clone()
		}
	}

	/// Gets the last value assigned to `self`, or returns an [`Error::UndefinedVariable`] if we
	/// haven't been assigned to yet.
	pub fn run(&self) -> Result<Value> {
		self.fetch().ok_or_else(|| Error::UndefinedVariable(self.name().clone()))
	}
}
