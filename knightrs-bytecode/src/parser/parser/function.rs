use crate::parser::Ast;
use crate::parser::AstKind;
use crate::parser::{ParseError, ParseErrorKind, Parseable, Parseable_OLD, Parser, VariableName};
use crate::program::{DeferredJump, JumpWhen};
use crate::strings::StringSlice;
use crate::vm::Opcode;

use super::SourceLocation;

pub struct Function;

fn simple_opcode_for(func: char) -> Option<Opcode> {
	match func {
		// arity 0
		'P' => Some(Opcode::Prompt),
		'R' => Some(Opcode::Random),

		// arity 1
		'C' => Some(Opcode::Call),
		'Q' => Some(Opcode::Quit),
		'D' => Some(Opcode::Dump),
		'O' => Some(Opcode::Output),
		'L' => Some(Opcode::Length),
		'!' => Some(Opcode::Not),
		'~' => Some(Opcode::Negate),
		'A' => Some(Opcode::Ascii),
		',' => Some(Opcode::Box),
		'[' => Some(Opcode::Head),
		']' => Some(Opcode::Tail),

		// arity 2
		'+' => Some(Opcode::Add),
		'-' => Some(Opcode::Sub),
		'*' => Some(Opcode::Mul),
		'/' => Some(Opcode::Div),
		'%' => Some(Opcode::Mod),
		'^' => Some(Opcode::Pow),
		'<' => Some(Opcode::Lth),
		'>' => Some(Opcode::Gth),
		'?' => Some(Opcode::Eql),

		// arity 3
		'G' => Some(Opcode::Get),

		// arity 4
		'S' => Some(Opcode::Set),

		_ => None,
	}
}

fn parse_argument(
	parser: &mut Parser<'_, '_>,
	start: &SourceLocation,
	fn_name: char,
	arg: usize,
) -> Result<(), ParseError> {
	match parser.parse_expression() {
		Err(err) if matches!(err.kind, ParseErrorKind::EmptySource) => {
			return Err(start.clone().error(ParseErrorKind::MissingArgument(fn_name, arg)));
		}
		other => other,
	}
}

fn parse_assignment(start: SourceLocation, parser: &mut Parser<'_, '_>) -> Result<(), ParseError> {
	parser.strip_whitespace_and_comments();

	match super::VariableName::parse(parser) {
		Err(err) if matches!(err.kind, ParseErrorKind::EmptySource) => {
			return Err(start.error(ParseErrorKind::MissingArgument('=', 1)));
		}
		Err(err) => return Err(err),
		Ok(Some(Ast { kind: AstKind::Variable(name), loc })) => {
			// try for a block, if so give it a name.
			parser.strip_whitespace_and_comments();
			if parser.peek().map_or(false, |c| c == 'B') {
				parser.strip_keyword_function();
				parse_block(start, parser, Some(name.clone()))?;
			} else {
				parse_argument(parser, &start, '=', 2)?;
			}
			// ew, cloning is not a good answer.
			let opts = (*parser.opts()).clone();
			unsafe { parser.compiler().set_variable(name, &opts) }.map_err(|err| loc.error(err))?;
		}
		Ok(Some(_)) => unreachable!("variable always returns variables"),
		Ok(None) => {
			#[cfg(feature = "extensions")]
			{
				todo!("Assign to OUTPUT, PROMPT, RANDOM, strings, $, and more.");
			}

			return Err(start.error(ParseErrorKind::CanOnlyAssignToVariables));
		}
	}

	Ok(())
}

fn parse_block(
	start: SourceLocation,
	parser: &mut Parser<'_, '_>,
	name: Option<VariableName>,
) -> Result<(), ParseError> {
	// TODO: improve blocks later on by not having to jump over their definitions always.
	let jump_after = parser.compiler().defer_jump(JumpWhen::Always);

	let jump_index = parser.compiler().jump_index();
	parse_argument(parser, &start, 'B', 1)?;
	unsafe {
		parser.compiler().opcode_without_offset(Opcode::Return);
		jump_after.jump_to_current(parser.compiler());
	}

	parser.compiler().push_constant(crate::value::Block::new(jump_index).into());

	parser.compiler().record_block(start, jump_index, name);
	Ok(())
}

unsafe impl Parseable_OLD for Function {
	fn parse(parser: &mut Parser<'_, '_>) -> Result<bool, ParseError> {
		// this should be reowrked ot allow for registering arbitrary functions, as it doesn't
		// support `X`s

		let (fn_name, full_name) = if let Some(fn_name) = parser.advance_if(char::is_uppercase) {
			(fn_name, parser.strip_keyword_function().unwrap_or_default())
		} else if let Some(chr) = parser.advance() {
			(chr, "")
		} else {
			return Ok(false);
		};

		let start = parser.location();

		// Handle opcodes without anything special
		if let Some(simple_opcode) = simple_opcode_for(fn_name) {
			debug_assert!(!simple_opcode.takes_offset()); // no simple opcodes take offsets

			for arg in 0..simple_opcode.arity() {
				parse_argument(parser, &start, fn_name, arg + 1)?;
			}

			unsafe {
				// todo: rename to simple opcode?
				parser.compiler.opcode_without_offset(simple_opcode);
			}

			return Ok(true);
		}

		// Non-simple ones
		match fn_name {
			';' => {
				parse_argument(parser, &start, fn_name, 1)?;
				unsafe {
					parser.compiler.opcode_without_offset(Opcode::Pop);
				}
				parse_argument(parser, &start, fn_name, 1)?;
				Ok(true)
			}
			'=' => parse_assignment(start, parser).and(Ok(true)),
			'B' => parse_block(start, parser, None).and(Ok(true)),
			'&' | '|' => {
				parse_argument(parser, &start, fn_name, 1)?;
				unsafe {
					parser.compiler().opcode_without_offset(Opcode::Dup);
				}
				let end = parser.compiler().defer_jump(if fn_name == '&' {
					JumpWhen::False
				} else {
					JumpWhen::True
				});
				unsafe {
					// delete the value we dont want
					parser.compiler().opcode_without_offset(Opcode::Pop);
				}
				parse_argument(parser, &start, fn_name, 2)?;
				unsafe {
					end.jump_to_current(parser.compiler());
				}
				Ok(true)
			}
			'I' => {
				parse_argument(parser, &start, fn_name, 1)?;
				let to_false = parser.compiler().defer_jump(JumpWhen::False);
				parse_argument(parser, &start, fn_name, 2)?;
				let to_end = parser.compiler().defer_jump(JumpWhen::Always);
				unsafe {
					to_false.jump_to_current(&mut parser.compiler());
				}
				parse_argument(parser, &start, fn_name, 3)?;
				unsafe {
					to_end.jump_to_current(parser.compiler());
				}
				Ok(true)
			}
			'W' => {
				let while_start = parser.compiler().jump_index();

				parse_argument(parser, &start, fn_name, 1)?;
				let deferred = parser.compiler().defer_jump(JumpWhen::False);
				parser.loops.push((while_start, vec![deferred]));

				parse_argument(parser, &start, fn_name, 3)?;
				unsafe {
					parser.compiler().jump_to(JumpWhen::Always, while_start);
				}

				// jump all `break`s to the end
				for deferred in parser.loops.pop().unwrap().1 {
					unsafe {
						deferred.jump_to_current(parser.compiler());
					}
				}

				Ok(true)
			}
			// TODO: extensions lol
			#[cfg(feature = "extensions")]
			'X' => match full_name {
				"BREAK" if parser.opts().extensions.syntax.control_flow => {
					let deferred = parser.compiler().defer_jump(JumpWhen::Always);
					parser
						.loops
						.last_mut()
						.expect("<todo: exception when `break` when nothing to break, or in a funciton?>")
						.1
						.push(deferred);
					Ok(true)
				}
				"CONTINUE" if parser.opts().extensions.syntax.control_flow => {
					let starting = parser
						.loops
						.last()
						.expect("<todo: exception when `break` when nothing to break, or in a funciton?>")
						.0;
					unsafe {
						parser.compiler().jump_to(JumpWhen::Always, starting);
					}
					Ok(true)
				}
				_ => Err(start.error(ParseErrorKind::UnknownExtensionFunction(full_name.to_string()))),
			},
			_ => todo!("invalid fn: {fn_name:?}"),
		}
	}
}
