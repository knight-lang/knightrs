use super::*;
use std::collections::HashMap;
use std::io;

/// The environment hosts all relevant information for knight programs.
pub struct Builder<'e> {
	stdin: Option<Box<Stdin<'e>>>,
	stdout: Option<Box<Stdout<'e>>>,
	options: Options,
	functions: HashMap<Character, &'e Function>,
	extensions: HashMap<Text, &'e Function>,

	#[cfg(feature = "system-function")]
	system: Option<Box<System<'e>>>,

	#[cfg(feature = "use-function")]
	read_file: Option<Box<ReadFile<'e>>>,
}

impl Default for Builder<'_> {
	fn default() -> Self {
		Self {
			stdin: None,
			stdout: None,
			options: Options::default(),
			functions: crate::function::default(),
			extensions: crate::function::extensions(),

			#[cfg(feature = "system-function")]
			system: None,

			#[cfg(feature = "use-function")]
			read_file: None,
		}
	}
}

impl<'e> Builder<'e> {
	pub fn stdin<S: BufRead + Send + Sync + 'e>(&mut self, stdin: S) {
		self.stdin = Some(Box::new(stdin) as Box<_>);
	}

	pub fn options(&mut self) -> &mut Options {
		&mut self.options
	}

	pub fn stdout<S: Write + Send + Sync + 'e>(&mut self, stdout: S) {
		self.stdout = Some(Box::new(stdout) as Box<_>);
	}

	pub fn functions(&mut self) -> &mut HashMap<Character, &'e Function> {
		&mut self.functions
	}

	pub fn extensions(&mut self) -> &mut HashMap<Text, &'e Function> {
		&mut self.extensions
	}

	#[cfg(feature = "system-function")]
	pub fn system<F>(&mut self, func: F)
	where
		F: FnMut(&TextSlice, Option<&TextSlice>) -> crate::Result<Text> + Send + Sync + 'e,
	{
		self.system = Some(Box::new(func) as Box<_>);
	}

	#[cfg(feature = "use-function")]
	pub fn read_file<F>(&mut self, func: F)
	where
		F: FnMut(&TextSlice) -> crate::Result<Text> + Send + Sync + 'e,
	{
		self.read_file = Some(Box::new(func) as Box<_>);
	}

	pub fn build(self) -> Environment<'e> {
		Environment {
			options: self.options,
			variables: HashSet::default(),
			stdin: self.stdin.unwrap_or_else(|| Box::new(io::BufReader::new(io::stdin()))),
			stdout: self.stdout.unwrap_or_else(|| Box::new(io::stdout())),

			#[cfg(feature = "system-function")]
			system: self.system.unwrap_or_else(|| {
				Box::new(|cmd, stdin| {
					use std::process::{Command, Stdio};

					assert!(stdin.is_none(), "todo, system function with non-default stdin");

					let output = Command::new("/bin/sh")
						.arg("-c")
						.arg(&**cmd)
						.stdin(Stdio::inherit())
						.output()
						.map(|out| String::from_utf8_lossy(&out.stdout).into_owned())?;

					Ok(Text::try_from(output)?)
				})
			}),

			#[cfg(feature = "use-function")]
			read_file: self.read_file.unwrap_or_else(|| {
				Box::new(|filename| Ok(std::fs::read_to_string(&**filename)?.try_into()?))
			}),

			extensions: self.extensions,
			functions: self.functions,
			rng: Box::new(StdRng::from_entropy()),

			#[cfg(feature = "assign-to-prompt")]
			prompt_lines: Default::default(),

			#[cfg(feature = "assign-to-system")]
			system_results: Default::default(),
		}
	}
}
