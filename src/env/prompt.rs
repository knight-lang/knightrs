use super::Environment;
use crate::value::{Runnable, Text, ToText, Value};
use crate::{Ast, Result};
use std::collections::VecDeque;
use std::io::{self, BufRead};

pub struct Prompt<'e> {
	pub(crate) default: Box<dyn BufRead + 'e + Send + Sync>,

	#[cfg(feature = "extensions")]
	replacement: Option<PromptReplacement<'e>>,
}

impl Default for Prompt<'_> {
	fn default() -> Self {
		Self {
			default: Box::new(io::BufReader::new(io::stdin())),

			#[cfg(feature = "extensions")]
			replacement: None,
		}
	}
}

#[cfg(feature = "extensions")]
enum PromptReplacement<'e> {
	Closed,
	Buffered(VecDeque<Text>),
	Computed(Ast<'e>),
}

fn strip_ending(line: &mut String) {
	match line.pop() {
		Some('\n') => {}
		Some('\r') => {}
		Some(other) => {
			line.push(other);
			return;
		}
		None => return,
	}

	loop {
		match line.pop() {
			Some('\r') => {}
			Some(other) => {
				line.push(other);
				return;
			}
			None => return,
		}
	}
}

impl<'e> Prompt<'e> {
	#[cfg(feature = "extensions")]
	pub fn close(&mut self) {
		self.replacement = Some(PromptReplacement::Closed);
	}

	// ie, set the thing that does the computation
	#[cfg(feature = "extensions")]
	pub fn set_ast(&mut self, ast: Ast<'e>) {
		self.replacement = Some(PromptReplacement::Computed(ast));
	}

	#[cfg(feature = "extensions")]
	pub fn reset_replacement(&mut self) {
		self.replacement = None;
	}

	#[cfg(feature = "extensions")]
	pub fn add_lines(&mut self, new_lines: &crate::value::text::TextSlice) {
		let lines = match self.replacement {
			Some(PromptReplacement::Buffered(ref mut lines)) => lines,
			_ => {
				self.replacement = Some(PromptReplacement::Buffered(Default::default()));
				match self.replacement {
					Some(PromptReplacement::Buffered(ref mut lines)) => lines,
					_ => unreachable!(),
				}
			}
		};

		for line in (&**new_lines).split('\n') {
			let mut line = line.to_string();
			strip_ending(&mut line);
			lines.push_back(line.try_into().unwrap());
		}
	}

	pub fn read_line(&mut self, env: &mut Environment<'e>) -> Result<Option<Text>> {
		#[cfg(feature = "extensions")]
		match self.replacement.as_mut() {
			None => {}
			Some(PromptReplacement::Closed) => return Ok(None),
			Some(PromptReplacement::Buffered(queue)) => return Ok(queue.pop_front()),
			Some(PromptReplacement::Computed(ast)) => {
				return match ast.run(env)? {
					Value::Null => Ok(None),
					other => Ok(Some(other.to_text()?)),
				}
			}
		}

		let mut line = String::new();

		// If we read an empty line, return null.
		if self.default.read_line(&mut line)? == 0 {
			return Ok(None);
		}

		strip_ending(&mut line);
		Ok(Some(Text::try_from(line)?))
	}
}
