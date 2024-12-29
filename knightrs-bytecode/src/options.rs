use crate::strings::Encoding;

#[derive(Default, Clone)]
pub struct Options {
	pub encoding: Encoding,

	#[cfg(feature = "compliance")]
	pub compliance: Compliance,

	#[cfg(feature = "extensions")]
	pub extensions: Extensions,

	#[cfg(feature = "qol")]
	pub qol: QualityOfLife,

	#[cfg(feature = "embedded")]
	pub embedded: Embedded,

	#[cfg(feature = "check-variables")]
	pub check_variables: bool,

	#[cfg(feature = "check-parens")]
	pub check_parens: bool, // TODO: also make this strict compliance
}

#[derive(Default, Clone)]
#[cfg(feature = "qol")]
pub struct QualityOfLife {
	pub stacktrace: bool,
}

#[derive(Default, Clone)]
#[cfg(feature = "embedded")]
pub struct Embedded {
	pub dont_exit_when_quitting: bool,
}

/// Options for additional compliance checking.
///
/// If `feature = "compliance"` is not specified, all of these are disabled.
#[derive(Default, Clone)]
#[cfg(feature = "compliance")]
pub struct Compliance {
	/// Ensure that [`KnString`] and [`List`]s have lengths no longer than [`i32::MAX`].
	///
	/// This usually doesn't happen during normal execution (as allocations that long are very rare),
	/// but can happen if `* str large_number` is used.
	///
	/// Note that this implementation always checks for lengths greater than [`isize::MAX`], as going
	/// beyond that can cause panics.
	pub check_container_length: bool, // make sure containers are within `i32::MAX`

	/// Ensures that all [`Integer`] are strictly within [`i32`]'s bounds, as per the knight specs.
	///
	/// This ensure that _every_ operation that could create an [`Integer`] (including integer
	/// literals, string conversions, `LENGTH` of collections, etc) are checked.
	///
	/// Using this but not [`check_container_length`](Compliance::check_container_length) can cause
	/// getting the `LENGTH` of containers to fail.
	pub i32_integer: bool,

	/// Checks all [`Integer`] math operations for over/underflow.
	///
	/// Without this, all operations will wrap around.
	pub check_overflow: bool,

	/// Check to make sure all arguments to [`Integer`]'s arithmetic functions are within bounds.
	///
	/// This enables checks for negative bases in [`Integer::remainder`] and negative exponents in
	/// [`Integer::power`]. (Note that zero divisors for [`Integer::divide`] and zero bases for
	/// [`Integer::remainder`] are always checked, regardless of this option.)
	pub check_integer_function_bounds: bool,

	/// Ensures all variables are at most [`VariableName::MAX_NAME_LEN`] bytes long.
	///
	/// Without this, variables can be any length.
	pub variable_name_length: bool,

	/// Ensures that at most [`crate::vm::MAX_VARIABLE_COUNT`] variables are used.
	///
	/// Without this, there's no limit on the amount of variables that can be used.
	///
	/// Note that if this is enabled, it'll also check [`Extensions::BuiltinFns::assign_to_strings`]
	/// to make sure that not too many variables are created.
	pub variable_count: bool,

	/// Ensure programs are a single expression.
	///
	/// Without this, trailing tokens are allowed, and are simply ignored.
	pub forbid_trailing_tokens: bool,

	/// Verify that [`Value::kn_equals`] is not called with [`Block`]s.
	pub check_equals_params: bool,
	pub no_block_conversions: bool,
	pub cant_dump_blocks: bool,

	pub limit_rand_range: bool,
	pub check_quit_status_codes: bool,
	pub disallow_negative_int_to_list: bool,
	pub disable_all_extensions: bool, // TODO
}

cfg_if! {
if #[cfg(feature = "extensions")] {
	#[derive(Default, Clone)]
	pub struct Extensions {
		pub builtin_fns: BuiltinFns,
		pub syntax: Syntax,
		pub types: Types,
		pub breaking: BreakingChanges,
		pub functions: Functions,
		pub negative_indexing: bool,
		pub argv: bool,
	}

	#[derive(Default, Clone)]
	pub struct Types {
		pub floats: bool, // not working, potential future idea.
		pub hashmaps: bool, // not working, potential future idea.
		pub classes: bool, // not working, potential future idea.
	}

	#[derive(Default, Clone)]
	pub struct Functions {
		pub eval: bool,
		pub value: bool,
	}

	#[derive(Default, Clone)]
	pub struct BreakingChanges {
		pub negate_reverses_collections: bool, // not working, potential future idea.
		pub random_can_be_negative: bool,
	}

	#[derive(Default, Clone)]
	pub struct Syntax {
		pub list_literals: bool, // not working
		pub string_interpolation: bool, // not working
		pub control_flow: bool, // XBREAK, XCONTINUE, XRETURN : partially working
	}

	#[derive(Default, Clone)]
	pub struct BuiltinFns {
		pub boolean: bool,
		pub string: bool,
		pub list: bool,
		pub integer: bool,
		pub null: bool,

		pub length_of_anything: bool,
		pub assign_to_strings: bool,
		pub assign_to_random: bool,
	}
}}
