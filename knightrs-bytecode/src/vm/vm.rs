use crate::gc::GarbageCollected;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::mem::MaybeUninit;

use super::{Opcode, RuntimeError};
use crate::gc::{AsValueInner, GcRoot};
use crate::parser::{SourceLocation, VariableName};
use crate::program::{JumpIndex, Program};
use crate::strings::KnStr;
use crate::value::{Block, Integer, KnString, List, ToBoolean, ToInteger, ToKnString, Value};
use crate::{Environment, Error};

pub struct Vm<'prog, 'src, 'path, 'env, 'gc> {
	program: &'prog Program<'src, 'path, 'gc>,
	env: &'env mut Environment<'gc>,
	current_index: usize,
	stack: Vec<Value<'gc>>,
	variables: Vec<Option<Value<'gc>>>, // It's a vec for extensions; without extensions this could be a box, but i see no need to

	#[cfg(feature = "stacktrace")]
	callstack: Vec<usize>,

	#[cfg(feature = "stacktrace")]
	known_blocks: HashMap<usize, VariableName<'src>>,

	#[cfg(feature = "extensions")]
	dynamic_variables: HashMap<VariableName<'static>, Value<'gc>>,
}

impl<'prog, 'src, 'path, 'env, 'gc> Vm<'prog, 'src, 'path, 'env, 'gc> {
	pub fn new(program: &'prog Program<'src, 'path, 'gc>, env: &'env mut Environment<'gc>) -> Self {
		Self {
			program,
			env,
			current_index: 0,
			stack: Vec::new(),
			variables: vec![None; program.num_variables()].into(),

			#[cfg(feature = "stacktrace")]
			callstack: Vec::new(),

			#[cfg(feature = "stacktrace")]
			known_blocks: HashMap::default(),

			#[cfg(feature = "extensions")]
			dynamic_variables: HashMap::default(),
		}
	}

	pub unsafe fn mark(&self) {
		unsafe {
			self.program.mark();
		}

		for value in self.stack.iter() {
			unsafe {
				value.mark();
			}
		}

		for var in self.variables.iter() {
			if let Some(value) = var {
				unsafe {
					value.mark();
				}
			}
		}

		#[cfg(feature = "extensions")]
		for value in self.dynamic_variables.values() {
			unsafe {
				value.mark();
			}
		}
	}

	pub fn run_entire_program(
		&mut self,
		argv: impl IntoIterator<Item = String>,
	) -> crate::Result<Value<'gc>> {
		#[cfg(feature = "extensions")]
		if self.env.opts().extensions.argv {
			let mut first = true;
			// self.env.gc().pause();
			let argv = argv
				.into_iter()
				.skip_while(|ele| {
					if first {
						first = false;
						ele == "--"
					} else {
						false
					}
				})
				.map(|str| {
					KnString::new(str, self.env.opts(), self.env.gc())
						.map(|string| unsafe { string.assume_used() }.into())
				})
				.collect::<Result<Vec<_>, _>>()?;

			let argv = List::new(argv, self.env.opts(), self.env.gc())
				.map(|l| unsafe { l.assume_used() }.into())?;

			// SAFETY: if extensions are enabled, argv is always added, regardless of whether or not it
			// was specified, so this is valid. Also, TODO: make sure `VALUE`, when implemented, fails
			// for undefined variables on `argv` if argv isn't set
			debug_assert_ne!(self.variables.len(), 0);
			unsafe {
				self.set_variable(crate::program::Compiler::ARGV_VARIABLE_INDEX, argv);
			}
			// self.env.gc().unpause();
		}

		self.run_entire_program_without_argv()
	}

	pub fn run_entire_program_without_argv(&mut self) -> crate::Result<Value<'gc>> {
		self.run(Block::new(JumpIndex(0)))
	}

	pub fn run(&mut self, block: Block) -> crate::Result<Value<'gc>> {
		// Save previous index
		let index = self.current_index;

		#[cfg(feature = "stacktrace")]
		self.callstack.push(self.current_index);

		// Used for debugging later
		#[cfg(debug_assertions)]
		let stack_len = self.stack.len();

		// Actually call the functoin
		self.current_index = block.inner().0;
		let result = self.run_inner();

		// Add the stacktrace to the lsit
		#[cfg(feature = "stacktrace")]
		let result = match result {
			Ok(ok) => Ok(ok),
			Err(todo @ crate::Error::Stacktrace(_)) => Err(todo),
			Err(err) => Err(crate::Error::Stacktrace(self.error(err).to_string())),
		};

		#[cfg(feature = "stacktrace")]
		{
			let result = self.callstack.pop();
			debug_assert_eq!(result, Some(index));
		}

		#[cfg(debug_assertions)]
		debug_assert_eq!(stack_len, self.stack.len(), "{:?}", result);

		self.current_index = index;

		result
	}

	pub fn error(&mut self, err: crate::Error) -> RuntimeError {
		RuntimeError {
			err,
			#[cfg(feature = "stacktrace")]
			stacktrace: self.stacktrace(),
			_ignored: (&(), &()),
		}
	}

	#[cfg(feature = "stacktrace")]
	pub fn stacktrace(&self) -> super::Stacktrace {
		use super::Callsite;

		super::Stacktrace::new(self.callstack.iter().map(|&idx| {
			let loc = self.program.source_location_at(idx);
			Callsite::new(self.block_name_at(idx), loc)
		}))
	}

	#[cfg(feature = "stacktrace")]
	fn block_name_at(&self, mut idx: usize) -> Option<VariableName> {
		while idx != 0 {
			if let Some(name) = self.known_blocks.get(&idx) {
				return Some(name.clone());
			}

			idx -= 1;
		}

		None
	}

	fn push_gcroot<T: AsValueInner + Into<Value<'gc>>>(&mut self, root: GcRoot<'gc, T>) {
		unsafe { root.with_inner(|inner| self.stack.push(inner.into())) }
	}

	fn run_inner(&mut self) -> crate::Result<Value<'gc>> {
		use std::mem::MaybeUninit;

		const NULL: MaybeUninit<Value<'_>> = MaybeUninit::uninit();

		#[cfg(not(feature = "stacktrace"))]
		let mut jumpstack = Vec::new();

		loop {
			// SAFETY: all programs are well-formed, so we know the current index is in bounds.
			let (opcode, offset) = unsafe { self.program.opcode_at(self.current_index) };
			self.current_index += 1;
			// println!("[{:3?}:{opcode:08?}] {:?} ({:?})", self.current_index, offset, self.stack);

			// Read arguments in
			unsafe {
				debug_assert!(opcode.arity() <= self.stack.len());

				// Pop the arguments off the stack. The remaining arguments are in `spare_capacity_mut`.
				// This does mean that we cannot modify `self.stack` until we've interacted with all the
				// individual arguments.
				self.stack.set_len(self.stack.len() - opcode.arity());
			}

			let args = self.stack.spare_capacity_mut();

			// Get the last argument on the stack. Requires an `unsafe` block in case the stack is
			// empty for some reason.
			macro_rules! last {
				() => {{
					debug_assert_ne!(self.stack.len(), 0);
					self.stack.last().unwrap_unchecked()
				}};
			}

			// Gets an argument from the argument stack
			macro_rules! arg {
				(&$idx:expr) => {{
					let idx = $idx;

					debug_assert!(idx < opcode.arity());
					// realistically shouldnt ever happen as args is also the values past the end too
					debug_assert!(idx <= args.len());

					args.get_unchecked(idx).assume_init_ref()
				}};

				($idx:expr) => {{
					let idx = $idx;

					debug_assert!(idx < opcode.arity());
					// realistically shouldnt ever happen as args is also the values past the end too
					debug_assert!(idx <= args.len());

					args.get_unchecked(idx).assume_init_read()
				}};
			}

			macro_rules! end {
				() => {
					args.get_unchecked_mut(0)
				};
			}

			macro_rules! push_no_resize {
				($value:expr) => {
					args.get_unchecked_mut(0).write($value);
					self.stack.set_len(self.stack.len() + 1);
				};
			}

			// NOTE: ALL OPCODES MUST ALWAYS EXTRACT THEIR ARGUMENTS EXACTLY ONCE FROM `args`,
			// else memory issues will crop up (such as memory leaks or double reads).
			match opcode {
				// Builtins
				Opcode::PushConstant => {
					// No need for a "target", as `self.program` is always GC'd.
					self.stack.push(unsafe { self.program.constant_at(offset) }.clone())
				}
				// SAFETY: program is well-defined, so jumps are always correct
				Opcode::Jump => unsafe { self.jump_to(offset) },
				Opcode::JumpIfTrue => {
					if unsafe { arg![0] }.to_boolean(self.env)? {
						// SAFETY: program is well-defined, so jumps are always correct
						unsafe { self.jump_to(offset) };
					}
				}
				Opcode::JumpIfFalse => {
					if !unsafe { arg![0] }.to_boolean(self.env)? {
						// SAFETY: program is well-defined, so jumps are always correct
						unsafe { self.jump_to(offset) }
					}
				}

				Opcode::GetVar => {
					let value = unsafe { self.get_variable(offset) }?;
					// gotta push, in case the stack isn't large enough
					self.stack.push(value);
				}

				Opcode::SetVar => {
					// SAFETY: construction of `Program`s guarantee that `SetVar` always has at least one
					// value on the stack (the value to assign)
					let value = unsafe { last!() }.clone();

					// SAFETY: construction of `Program`s guarantees that `SetVar` will have an offset,
					// and that it's a a valid variable index.
					unsafe { self.set_variable(offset, value) };
				}

				#[cfg(feature = "extensions")]
				Opcode::SetDynamicVar => {
					let value = unsafe { arg![1] }; // read in case `.to_kstring` in the next line modifies args
					let name = unsafe { arg![0] }.to_knstring(self.env)?;
					let varname = VariableName::new(&name, self.env.opts())
						.map_err(|err| crate::Error::Todo(err.to_string()))?;

					// If it already exists, then just use that
					if let Some(index) = self.program.variable_index(&varname) {
						unsafe {
							self.set_variable(index, value.clone());
						}
					} else {
						self.dynamic_variables.insert(varname.become_owned(), value.clone());
					}

					// TODO: Can this be replaced with an `&mut MaybeUninit`?
					self.stack.push(value);
				}

				Opcode::SetVarPop => todo!(), //self.variables[offset] = unsafe{arg![0]}.clone(),

				// Arity 0
				Opcode::Prompt => {
					// Can't use `MaybeUninit` in case we're at the end of the stack.
					if let Some(prompted) = self.env.prompt()? {
						self.push_gcroot(prompted);
					} else {
						self.stack.push(Value::NULL);
					}
				}
				// Can't use `MaybeUninit` in case we're at the end of the stack.
				Opcode::Random => self.stack.push(self.env.random()?.into()),

				Opcode::Dup => self.stack.push(unsafe { last!() }.clone()),

				// SAFETY: `function.rs` special-cases `DUMP` to ensure it has something, even tho
				// its arity is 0
				Opcode::Dump => unsafe { last!() }.kn_dump(self.env)?,

				// Arity 1
				#[cfg(feature = "stacktrace")]
				Opcode::Return => return Ok(unsafe { arg![0] }),

				#[cfg(not(feature = "stacktrace"))]
				Opcode::Return => {
					// There's somewhere to jump to, go there.
					if let Some(ip) = jumpstack.pop() {
						likely_stable::likely(true);
						unsafe { self.jump_to(ip) };
					} else {
						// There's nowhere to jump to, return the block of code.
						return Ok(self.stack.pop().unwrap_or_else(|| {
							#[cfg(debug_assertions)]
							unreachable!("<bug: pop when nothing left>");
							unsafe { std::hint::unreachable_unchecked() }
						}));
					}
				}

				Opcode::Call => match unsafe { arg![0] } {
					#[cfg(not(feature = "stacktrace"))]
					Value::Block(block) => {
						likely_stable::likely(true);
						jumpstack.push(self.current_index);
						unsafe { self.jump_to(block.inner().0) };
					}
					other => {
						let value = other.kn_call(self)?;
						self.stack.push(value);
					}
				},

				Opcode::Quit => {
					let status = unsafe { arg![0] }.to_integer(self.env)?;
					let status = i32::try_from(status.inner()).expect("todo: out of bounds for i32");
					self.env.quit(status)?;
				}
				Opcode::Output => {
					use std::io::Write;
					let kstring = unsafe { arg![0] }.to_knstring(self.env)?;
					let strref = kstring.as_str();

					let mut output = self.env.output();

					if let Some(stripped) = strref.strip_suffix('\\') {
						write!(output, "{stripped}")
					} else {
						writeln!(output, "{strref}")
					}
					.map_err(|err| Error::IoError { func: "OUTPUT", err })?;
					let _ = output.flush(); // explicitly ignore errors with flushing

					// SAFETY: `Output` is guaranteed to be given an argument. We've also already
					// read from it.z
					unsafe {
						push_no_resize!(Value::NULL);
					}
				}
				Opcode::Length => {
					let value = unsafe { arg![0] }.kn_length(self.env)?.into();
					unsafe {
						push_no_resize!(value);
					}
				}
				Opcode::Not => unsafe {
					// TODO: should `kn_not` even exist?
					arg![0].kn_not(end!(), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Negate => unsafe {
					arg![0].kn_negate(end!(), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Ascii => unsafe {
					arg![0].kn_ascii(end!(), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Box => {
					let boxed = List::boxed(unsafe { arg![0] }.clone(), self.env.gc());
					self.push_gcroot(boxed);
				}
				Opcode::Head => unsafe {
					arg![0].kn_head(end!(), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Tail => unsafe {
					arg![0].kn_tail(end!(), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Pop => continue, /* do nothing, the arity already popped */

				Opcode::Add => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_plus(&rhs, &mut start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Sub => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_minus(&rhs, start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Mul => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_asterisk(&rhs, start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Div => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_slash(&rhs, start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Mod => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_percent(&rhs, start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Pow => unsafe {
					let (start, rest) = args.split_at_mut_unchecked(1);
					let value = start.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let rhs = rest.get_unchecked(0).assume_init_read();
					value.kn_caret(&rhs, start.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},
				Opcode::Lth => {
					let value = (unsafe { arg![0] }.kn_compare(&unsafe { arg![1] }, "<", self.env)?
						== Ordering::Less)
						.into();
					unsafe {
						push_no_resize!(value);
					}
				}
				Opcode::Gth => {
					let value = (unsafe { arg![0] }.kn_compare(&unsafe { arg![1] }, ">", self.env)?
						== Ordering::Greater)
						.into();
					unsafe {
						push_no_resize!(value);
					}
				}

				Opcode::Eql => {
					let value = (unsafe { arg![0] }.kn_equals(&unsafe { arg![1] }, self.env)?).into();
					unsafe {
						push_no_resize!(value);
					}
				}

				Opcode::Get => unsafe {
					let (first, rest) = args.split_at_mut_unchecked(1);
					let value = first.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let start = rest.get_unchecked(0).assume_init_read();
					let length = rest.get_unchecked(1).assume_init_read();
					value.kn_get(&start, &length, first.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},

				Opcode::Set => unsafe {
					let (first, rest) = args.split_at_mut_unchecked(1);
					let value = first.get_unchecked(0).assume_init_read(); // read it so we can target it with `kn_plus`
					let start = rest.get_unchecked(0).assume_init_read();
					let length = rest.get_unchecked(1).assume_init_read();
					let repl = rest.get_unchecked(2).assume_init_read();
					value.kn_set(&start, &length, &repl, first.get_unchecked_mut(0), self.env)?;
					self.stack.set_len(self.stack.len() + 1);
				},

				// EXTENSIONS
				#[cfg(feature = "extensions")]
				Opcode::AssignDynamic => match offset {
					_ if offset == super::opcode::DynamicAssignment::Random as _ => {
						let seed = unsafe { last!() }.to_integer(self.env)?;
						self.env.seed_random(seed);
					}
					_ => todo!("{:?}", offset),
				},

				// TODO: the `vm` evals in its entirely own vm, which isnt what we wnat
				#[cfg(feature = "extensions")]
				Opcode::Eval => {
					let program = unsafe { arg![0] }.to_knstring(self.env)?;
					let mut parser = crate::parser::Parser::new(&mut self.env, None, program.as_str())?;
					let program = parser.parse_program()?;
					let value = Vm::new(&program, self.env).run_entire_program_without_argv()?;
					unsafe {
						push_no_resize!(value);
					}
				}

				#[cfg(feature = "extensions")]
				Opcode::Value => {
					let variable_name = unsafe { arg![0] }.to_knstring(self.env)?;

					let varname = VariableName::new(&variable_name, self.env.opts())
						.map_err(|err| crate::Error::Todo(err.to_string()))?;

					let value = if let Some(compiletime_variable_offset) =
						self.program.variable_index(&varname)
					{
						// SAFETY: `variable_index` ensures it always returns a valid index., i think
						unsafe { self.get_variable(offset)? }
					} else {
						self
							.dynamic_variables
							.get(&varname)
							.ok_or_else(|| crate::Error::UndefinedVariable(varname.become_owned()))?
							.clone()
					};
					self.stack.push(value);
				}
			}
		}
	}

	// SAFETY: offset must be a valid place to jump to
	unsafe fn jump_to(&mut self, offset: usize) {
		self.current_index = offset
	}

	// SAFETY: the `offset` must be a valid variable offset
	unsafe fn get_variable(&mut self, offset: usize) -> crate::Result<Value<'gc>> {
		debug_assert!(offset <= self.variables.len());

		unsafe { self.variables.get_unchecked(offset) }.clone().ok_or_else(|| {
			crate::Error::UndefinedVariable(self.program.variable_name(offset).clone().become_owned())
		})
	}

	// SAFETY: the `offset` must be a valid variable offset
	unsafe fn set_variable(&mut self, offset: usize, value: Value<'gc>) {
		debug_assert!(offset <= self.variables.len());

		// TODO: rework how stacktraces work
		#[cfg(feature = "stacktrace")]
		if let Some(ref block) = value.as_block() {
			let varname = self.program.variable_name(offset);
			self.known_blocks.insert(block.inner().0, varname.clone());
		}

		*unsafe { self.variables.get_unchecked_mut(offset) } = Some(value);
	}
}
