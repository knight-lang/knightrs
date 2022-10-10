use crate::parse::{ParseFn, Parser};
use crate::value::text::Character;
use crate::value::Runnable;
#[cfg(feature = "extensions")]
use crate::value::Text;
use crate::{Function, Integer, RefCount, Result, TextSlice, Value};
use rand::{rngs::StdRng, SeedableRng};
use std::collections::{HashMap, HashSet};

mod builder;
pub mod flags;
pub mod output;
pub mod prompt;
mod variable;
pub use builder::Builder;
pub use flags::Flags;
use output::Output;
use prompt::Prompt;
pub use variable::{IllegalVariableName, Variable};

#[cfg(feature = "extensions")]
type System<'e> = dyn FnMut(&TextSlice, Option<&TextSlice>) -> Result<Text> + 'e + Send + Sync;

#[cfg(feature = "extensions")]
type ReadFile<'e> = dyn FnMut(&TextSlice) -> Result<Text> + 'e + Send + Sync;

/// The environment hosts all relevant information for knight programs.
pub struct Environment<'e> {
	flags: Flags,
	variables: HashSet<Variable<'e>>,
	prompt: Prompt<'e>,
	output: Output<'e>,
	functions: HashMap<Character, &'e Function<'e>>,
	rng: StdRng,

	// Parsers are only modifiable when the `extensions` feature is enabled. Otherwise, the normal
	// set of parsers is loaded up.
	parsers: Vec<RefCount<dyn ParseFn<'e>>>,

	// A List of extension functions.
	#[cfg(feature = "extensions")]
	extensions: HashSet<&'e Function<'e>>,

	// A queue of things that'll be read from for `` ` `` instead of stdin.
	#[cfg(feature = "extensions")]
	system_results: std::collections::VecDeque<Text>,

	#[cfg(feature = "extensions")]
	system: Box<System<'e>>,

	#[cfg(feature = "extensions")]
	read_file: Box<ReadFile<'e>>,
}

#[cfg(feature = "multithreaded")]
sa::assert_impl_all!(Environment<'_>: Send, Sync);

impl Default for Environment<'_> {
	fn default() -> Self {
		Self::builder().build()
	}
}

impl<'e> Environment<'e> {
	pub fn builder() -> Builder<'e> {
		Builder::default()
	}

	/// Parses and executes `source` as knight code.
	pub fn play(&mut self, source: &TextSlice) -> Result<Value<'e>> {
		Parser::new(source, self).parse_program()?.run(self)
	}

	pub fn flags(&self) -> &Flags {
		&self.flags
	}

	pub fn functions(&self) -> &HashMap<Character, &'e Function<'e>> {
		&self.functions
	}

	pub fn parsers(&self) -> &[RefCount<dyn ParseFn<'e>>] {
		&self.parsers
	}

	pub fn prompt(&mut self) -> &mut Prompt<'e> {
		&mut self.prompt
	}

	pub fn output(&mut self) -> &mut Output<'e> {
		&mut self.output
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

		let variable = Variable::new(name.into(), self.flags())?;
		self.variables.insert(variable.clone());
		Ok(variable)
	}

	/// Gets a random `Integer`.
	pub fn random(&mut self) -> Integer {
		Integer::random(&mut self.rng, &self.flags)
	}

	/// Seeds the random number generator.
	#[cfg(feature = "extensions")]
	pub fn srand(&mut self, seed: Integer) {
		self.rng = StdRng::seed_from_u64(i64::from(seed) as u64)
	}

	/// Executes `command` as a shell command, returning its result.
	#[cfg(feature = "extensions")]
	pub fn run_command(&mut self, command: &TextSlice, stdin: Option<&TextSlice>) -> Result<Text> {
		(self.system)(command, stdin)
	}

	/// Gets the list of known extension functions.
	#[cfg(feature = "extensions")]
	pub fn extensions(&self) -> &HashSet<&'e Function<'e>> {
		&self.extensions
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
	pub fn read_file(&mut self, filename: &TextSlice) -> Result<Text> {
		(self.read_file)(filename)
	}
}
