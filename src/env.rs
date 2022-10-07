use crate::value::text::Character;
use crate::value::Runnable;
use crate::{Function, Integer, Result, Text, TextSlice, Value};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};

mod builder;
mod flags;
mod prompt;
mod variable;
pub use builder::Builder;
pub use flags::Flags;
pub use prompt::Prompt;
pub use variable::{IllegalVariableName, Variable};

type Stdout<'e> = dyn Write + 'e + Send + Sync;

#[cfg(feature = "extensions")]
type System<'e> = dyn FnMut(&TextSlice, Option<&TextSlice>) -> Result<Text> + 'e + Send + Sync;

#[cfg(feature = "extensions")]
type ReadFile<'e> = dyn FnMut(&TextSlice) -> Result<Text> + 'e + Send + Sync;

/// The environment hosts all relevant information for knight programs.
pub struct Environment<'e> {
	// We use a `HashSet` because we want the variable to own its name, which a `HashMap`
	// wouldn't allow for. (or would have redundant allocations.)
	variables: HashSet<Variable<'e>>,
	prompt: Prompt<'e>,
	stdout: Box<Stdout<'e>>,
	rng: Box<StdRng>,

	functions: HashMap<Character, &'e Function>,
	extensions: HashMap<Text, &'e Function>,

	// A queue of things that'll be read from for `` ` `` instead of stdin.
	#[cfg(feature = "extensions")]
	system_results: std::collections::VecDeque<Text>,

	#[cfg(feature = "extensions")]
	system: Box<System<'e>>,

	#[cfg(feature = "extensions")]
	read_file: Box<ReadFile<'e>>,

	flags: Flags,
}

#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Environment: Send, Sync);

impl Default for Environment<'_> {
	fn default() -> Self {
		Builder::default().build()
	}
}

impl<'e> Environment<'e> {
	/// Parses and executes `source` as knight code.
	pub fn play(&mut self, source: &TextSlice) -> Result<Value<'e>> {
		crate::Parser::new(source, self).parse_program()?.run(self)
	}

	pub fn flags(&self) -> &Flags {
		&self.flags
	}

	pub fn functions(&self) -> &HashMap<Character, &'e Function> {
		&self.functions
	}

	pub fn prompt(&mut self) -> &mut Prompt<'e> {
		&mut self.prompt
	}

	pub fn stdout(&mut self) -> &mut dyn Write {
		&mut self.stdout
	}

	/// Fetches the variable corresponding to `name` in the environment, creating one if it's the
	/// first time that name has been requested
	pub fn lookup(
		&mut self,
		name: &TextSlice,
	) -> std::result::Result<Variable<'e>, IllegalVariableName> {
		// OPTIMIZE: This does a double lookup, which isnt spectacular.
		if let Some(var) = self.variables.get(name) {
			return Ok(var.clone());
		}

		let variable = Variable::new(name.into())?;
		self.variables.insert(variable.clone());
		Ok(variable)
	}

	/// Gets a random `Integer`.
	pub fn random(&mut self) -> Integer {
		let rand = self.rng.gen::<i32>().abs();

		Integer::from(if cfg!(feature = "strict-compliance") { rand & 0x7fff } else { rand })
	}

	/// Seeds the random number generator.
	pub fn srand(&mut self, seed: Integer) {
		*self.rng = StdRng::seed_from_u64(i64::from(seed) as u64)
	}

	/// Executes `command` as a shell command, returning its result.
	#[cfg(feature = "extensions")]
	pub fn run_command(&mut self, command: &TextSlice, stdin: Option<&TextSlice>) -> Result<Text> {
		(self.system)(command, stdin)
	}

	/// Gets the list of known extension functions.
	pub fn extensions(&self) -> &HashMap<Text, &'e Function> {
		&self.extensions
	}

	/// Gets a mutable list of known extension functions, so you can add to them.
	pub fn extensions_mut(&mut self) -> &mut HashMap<Text, &'e Function> {
		&mut self.extensions
	}

	#[cfg(feature = "extensions")]
	pub fn add_to_system(&mut self, output: Text) {
		self.system_results.push_back(output);
	}

	#[cfg(feature = "extensions")]
	pub fn get_next_system_result(&mut self) -> Option<Text> {
		self.system_results.pop_front()
	}

	#[cfg(feature = "extensions")]
	pub fn read_file(&mut self, filename: &TextSlice) -> crate::Result<Text> {
		(self.read_file)(filename)
	}
}
