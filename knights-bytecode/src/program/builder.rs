use super::{DeferredJump, InstructionAndOffset, JumpIndex, JumpWhen, Program};
use crate::options::Options;
use crate::parser::SourceLocation;
use crate::strings::StringSlice;
use crate::value::{KString, Value};
use crate::vm::{Opcode, ParseErrorKind};

use std::collections::HashMap;

/// A Builder is used to construct [`Program`]s, which are then run via the [`Vm`](crate::Vm).
#[derive(Default)]
pub struct Builder {
	// The current code so far; The bottom-most byte is the opcode, and when that's shifted away, the
	// remainder is the offset.
	code: Vec<InstructionAndOffset>,

	// All the constants that've been declared so far. Used with [`Opcode::PushConstant`].
	constants: Vec<Value>,

	// The list of all variables encountered so far. (They're stored in an ordered set, as their
	// index is the "offset" that all `Opcodes` that interact with variables (eg [`Opcode::GetVar`])
	// will use.)
	variables: indexmap::IndexSet<Box<StringSlice>>,

	// Only enabled when stacktrace printing is enabled, this is a map from the bytecode offset (ie
	// the index into `code`) to a source location; Only the first bytecode from each line is added,
	// so when looking up in the `source_lines`, you need to
	#[cfg(feature = "stacktrace")]
	source_lines: HashMap<usize, SourceLocation>,

	// Only enabled when stacktrace printing is enabled, this is a mapping of jump indices (which
	// correspond to the first instruction of a [`Block`]) to the (optional) name of the block, and
	// the location where the block was declared.
	#[cfg(feature = "stacktrace")]
	block_locations: HashMap<JumpIndex, (Option<KString>, SourceLocation)>,
}

fn code_from_opcode_and_offset(opcode: Opcode, offset: usize) -> InstructionAndOffset {
	opcode as InstructionAndOffset | (offset as InstructionAndOffset) << 0o10
}

impl Builder {
	/// Finished building the [`Program`], and returns it
	///
	/// # Safety
	/// The caller must ensure that the "program" that has been designed will have exactly one new
	/// value on top of its stack whenever it returns, which is the return value of the program.
	///
	/// Additionally, the caller must enure that all deferred jumps have been `jump_to`'d
	pub unsafe fn build(mut self) -> Program {
		// SAFETY: The caller guarantees that we'll always have exactly one opcode on the top when
		// the program is finished executing, so we know
		unsafe {
			self.opcode_without_offset(Opcode::Return);
		}

		#[cfg(debug_assertions)]
		for &opcode in self.code.iter() {
			debug_assert_ne!(opcode, 0, "deferred jump which was never un-deferred encountered.")
		}

		Program {
			code: self.code.into(),
			constants: self.constants.into(),
			num_variables: self.variables.len(),

			#[cfg(feature = "stacktrace")]
			source_lines: self.source_lines,

			#[cfg(feature = "stacktrace")]
			block_locations: self.block_locations,

			#[cfg(debug_assertions)]
			variable_names: self.variables.into_iter().collect(),
		}
	}

	/// Gets the current index for the program, for use later on with jumps.
	pub fn jump_index(&self) -> JumpIndex {
		JumpIndex(self.code.len())
	}

	/// Indicates that a new line of code, located at `loc`, is about to begin. Used for stacktraces.
	#[cfg(feature = "stacktrace")]
	pub fn record_source_location(&mut self, loc: SourceLocation) {
		self.source_lines.insert(self.code.len(), loc);
	}

	/// Indicates that at the offset `whence`, a block named `name` with the source location `loc`
	/// exists. Used for stacktraces.
	#[cfg(feature = "stacktrace")]
	pub fn record_block(&mut self, loc: SourceLocation, whence: JumpIndex, name: Option<KString>) {
		self.block_locations.insert(whence, (name, loc));
	}

	/// Writes a jump to `index`, which will only be run if `when` is valid.
	///
	/// This is equivalent to calling `defer_jump` and then immediately calling `jump_to` on it.
	///
	/// # Safety
	/// `index` has to be a valid location to jump to within the program. (This means, but isn't
	/// limited to, jumping out of bounds, or jumping right before a destructive operation like `Add`
	/// isn't allowed. TODO: what other operations are illegal?)
	pub unsafe fn jump_to(&mut self, when: JumpWhen, index: JumpIndex) {
		// SAFETY: TODO
		unsafe { self.defer_jump(when).jump_to(self, index) };
	}

	/// Defers a jump when `when` is complete.
	///
	/// Note that while this isn't
	pub fn defer_jump(&mut self, when: JumpWhen) -> DeferredJump {
		let deferred = self.code.len();
		self.code.push(0);
		DeferredJump(deferred, when)
	}

	// SAFETY: `opcode` must take an offset and `offset` must be a valid offset for it.
	unsafe fn opcode_with_offset(&mut self, opcode: Opcode, offset: usize) {
		// No need to check if `offset as InstructionAndOffset`'s topbit is nonzero, as that's so massive it'll never happen
		self.code.push(code_from_opcode_and_offset(opcode, offset))
	}

	// SAFETY: `opcode` mustn't take an offset
	pub unsafe fn opcode_without_offset(&mut self, opcode: Opcode) {
		self.code.push(code_from_opcode_and_offset(opcode, 0)) // any offset'll do, it's ignored
	}

	pub fn push_constant(&mut self, value: Value) {
		let index = match self.constants.iter().enumerate().find(|(_, v)| value == **v) {
			Some((index, _)) => index,
			None => {
				let i = self.constants.len();
				self.constants.push(value);
				i
			}
		};

		// SAFETY: we know that `index` is a valid constant cause we just checked
		unsafe {
			self.opcode_with_offset(Opcode::PushConstant, index);
		}
	}

	fn variable_index(
		&mut self,
		name: &StringSlice,
		opts: &Options,
	) -> Result<usize, ParseErrorKind> {
		#[cfg(feature = "compliance")]
		if opts.compliance.variable_name_length && name.len() > crate::parser::MAX_VARIABLE_LEN {
			return Err(ParseErrorKind::VariableNameTooLong(name.to_owned()));
		}

		// TODO: check for name size (also in `set`)
		match self.variables.get_index_of(name) {
			Some(index) => Ok(index),
			None => {
				let i = self.variables.len();

				#[cfg(feature = "compliance")]
				if opts.compliance.variable_count && i > crate::vm::MAX_VARIABLE_COUNT {
					return Err(ParseErrorKind::TooManyVariables);
				}

				// TODO: check `name` variable len
				self.variables.insert(name.into_boxed());
				Ok(i)
			}
		}
	}

	pub fn get_variable(
		&mut self,
		name: &StringSlice,
		opts: &Options,
	) -> Result<(), ParseErrorKind> {
		let index = self.variable_index(name, opts)?;

		unsafe {
			self.opcode_with_offset(Opcode::GetVar, index);
		}

		Ok(())
	}

	// SAFETY: when called, a value has to be on the stack
	pub unsafe fn set_variable(
		&mut self,
		name: &StringSlice,
		opts: &Options,
	) -> Result<(), ParseErrorKind> {
		let index = self.variable_index(name, opts)?;

		unsafe {
			self.opcode_with_offset(Opcode::SetVar, index);
		}

		Ok(())
	}

	// SAFETY: when called, a value has to be on the stack
	pub unsafe fn set_variable_pop(
		&mut self,
		name: &StringSlice,
		opts: &Options,
	) -> Result<(), ParseErrorKind> {
		let index = self.variable_index(name, opts)?;

		unsafe {
			self.opcode_with_offset(Opcode::SetVarPop, index);
		}

		Ok(())
	}
}

impl DeferredJump {
	pub unsafe fn jump_to_current(self, builder: &mut Builder) {
		// SAFETY: TODO
		unsafe { self.jump_to(builder, builder.jump_index()) }
	}

	pub unsafe fn jump_to(self, builder: &mut Builder, index: JumpIndex) {
		assert_eq!(0, builder.code[self.0]);

		let opcode = match self.1 {
			JumpWhen::True => Opcode::JumpIfTrue,
			JumpWhen::False => Opcode::JumpIfFalse,
			JumpWhen::Always => Opcode::Jump,
		};

		builder.code[self.0] = code_from_opcode_and_offset(opcode, index.0);
	}
}
