use anyhow::{anyhow, Result};
use logos::Logos;
use std::collections::{HashMap, HashSet, VecDeque};

use self::parser::expression;

use super::{base_abstractions::*, lexer::Token};

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum PreprocessorQuotationStyle {
	AngleBrackets,
	DoubleQuotes,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorBinOp {
	Or,
	Xor,
	And,
	Plus,
	Minus,
	Times,
	Divide,
	Modulo,
	Equals,
	NotEquals,
	LogicalOr,
	LogicalAnd,
	LessThan,
	LessOrEqual,
	GreaterThan,
	GreaterOrEqual,
	BitwiseShiftLeft,
	BitwiseShiftRight,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorExpression {
	// TODO: the spec also allows malformed expressions, if they're skipped by conditional inclusion
	IntLiteral(i64),
	Identifier(String),
	BinOp(PreprocessorBinOp, Box<PreprocessorExpression>, Box<PreprocessorExpression>),
	Not(Box<PreprocessorExpression>),
	Defined(String),
}

pub type PreprocessorValue = i64;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorDirective {
	Include(PreprocessorQuotationStyle, String),
	If(PreprocessorExpression),
	ElseIf(PreprocessorExpression),
	Else,
	EndIf,
	Define(String, String),
	Undef(String),
	Pragma(String),
	Other(String, String),
}

mod parser {
	// The exp* functions match the C operator precedences, see
	// https://en.cppreference.com/w/c/language/operator_precedence
	use super::*;
	use nom::{
		branch::alt,
		bytes::complete::tag,
		character::complete::{alpha1, alphanumeric1, char, multispace0, multispace1, one_of},
		combinator::{complete, fail, map, map_res, recognize},
		multi::{fold_many1, many0, many0_count, many1},
		sequence::{delimited, pair, preceded, terminated, tuple},
		IResult,
	};

	fn ws<'a, F, O, E: nom::error::ParseError<&'a str>>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
	where
		F: FnMut(&'a str) -> IResult<&'a str, O, E>,
	{
		delimited(multispace0, inner, multispace0)
	}

	pub(super) fn identifier(input: &str) -> IResult<&str, &str> {
		recognize(pair(alt((alpha1, tag("_"))), many0_count(alt((alphanumeric1, tag("_"))))))(input)
	}

	fn decimal(input: &str) -> IResult<&str, i64> {
		map_res(recognize(many1(terminated(one_of("0123456789"), many0(char('_'))))), |out: &str| {
			str::replace(out, "_", "").parse()
		})(input)
	}

	fn hexadecimal(input: &str) -> IResult<&str, i64> {
		map_res(
			preceded(
				alt((tag("0x"), tag("0X"))),
				recognize(many1(terminated(one_of("0123456789abcdefABCDEF"), many0(char('_'))))),
			),
			|out: &str| i64::from_str_radix(&str::replace(out, "_", ""), 16),
		)(input)
	}

	fn integer(input: &str) -> IResult<&str, i64> {
		// TODO: binary
		alt((hexadecimal, decimal))(input)
	}

	fn defined(input: &str) -> IResult<&str, &str> {
		alt((
			delimited(tag("defined("), ws(identifier), tag(")")),
			preceded(tuple((tag("defined"), multispace1)), identifier),
		))(input)
	}

	pub(super) fn bin_op<'a>(
		lhs: &PreprocessorExpression,
		subexpr: fn(&str) -> IResult<&str, PreprocessorExpression>,
		variants: &[(&'static str, PreprocessorBinOp)],
	) -> impl FnMut(&'a str) -> IResult<&'a str, PreprocessorExpression> {
		use std::rc::Rc;
		let lhs = lhs.clone();
		let mut f: Rc<dyn Fn(&'a str) -> IResult<&'a str, PreprocessorBinOp>> = Rc::new(fail);
		for (sym, op) in variants {
			let sym: &'static str = sym;
			let op = op.clone();
			f = Rc::new(move |s| {
				let res: IResult<&'a str, PreprocessorBinOp> = tag(sym)(s).map(|(rest, _)| (rest, op.clone()));
				res.or_else(|_| f(s))
			});
		}

		move |input| {
			fold_many1(
				tuple((delimited(multispace0, &*f, multispace0), subexpr)),
				|| lhs.clone(),
				|l, (op, r)| PreprocessorExpression::BinOp(op, l.into(), r.into()),
			)(input)
		}
	}

	pub(super) fn factor(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorExpression::*;
		alt((
			delimited(tag("("), exp12, tag(")")),
			map(terminated(defined, multispace0), |ident| Defined(ident.to_string())),
			map(terminated(identifier, multispace0), |ident| Identifier(ident.to_string())),
			map(terminated(integer, multispace0), IntLiteral),
		))(input)
	}

	pub(super) fn exp3(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, f) = factor(input)?;

		bin_op(&f, factor, &[("*", Times), ("/", Divide), ("%", Modulo)])(input).or(Ok((input, f)))
	}

	pub(super) fn exp4(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp3)(input)?;

		bin_op(&e, exp3, &[("+", Plus), ("-", Minus)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp5(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp4)(input)?;

		bin_op(&e, exp4, &[("<<", BitwiseShiftLeft), (">>", BitwiseShiftRight)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp6(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp5)(input)?;

		bin_op(&e, exp5, &[("<=", LessOrEqual), ("<", LessThan), (">=", GreaterOrEqual), (">", GreaterThan)])(input)
			.or(Ok((input, e)))
	}

	pub(super) fn exp7(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp6)(input)?;

		bin_op(&e, exp6, &[("==", Equals), ("!=", NotEquals)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp8(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp7)(input)?;

		bin_op(&e, exp7, &[("&", And)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp9(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp8)(input)?;

		bin_op(&e, exp8, &[("^", Xor)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp10(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp9)(input)?;

		bin_op(&e, exp9, &[("|", Or)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp11(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp10)(input)?;

		bin_op(&e, exp10, &[("&&", LogicalAnd)])(input).or(Ok((input, e)))
	}

	pub(super) fn exp12(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		let (input, e) = ws(exp11)(input)?;

		bin_op(&e, exp11, &[("||", LogicalOr)])(input).or(Ok((input, e)))
	}

	pub fn expression(input: &str) -> IResult<&str, PreprocessorExpression> { complete(ws(exp12))(input) }
}

// TODO: is taking ownership necessary?
pub fn parse_pp_expression(buf: String) -> Option<PreprocessorExpression> {
	match parser::expression(&buf) {
		Ok((_, expr)) => Some(expr),
		Err(e) => {
			dbg!(e);
			None
		}
	}
}

pub type ResolvedToken = (FileId, PreprocessorDirective, Span);

// TODO: paths of FileId's? Also maybe it's better to work with (FileId, Span)
// necessary for "included from"
pub struct PreprocessorState<'a> {
	definitions: HashMap<String, String>,
	pub errors: Vec<((FileId, Span), String)>, // TODO: we should do better than strings here
	state: HashMap<FileId, VertexState>,
	/// A stack of branches for conditional compilation.
	/// This will contain the opening #if's and #ifdef's, popped when reaching #endif.
	conditional_stack: Vec<(ResolvedToken, Vec<ResolvedToken>, bool)>,
	lex: Box<LexFn<'a>>,
	to_id: Box<dyn FnMut(&str) -> FileId + 'a>,
}

type LexFn<'a> = dyn FnMut(FileId, &str) -> Option<&'a Vec<(Token, Span)>> + 'a;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
enum VertexState {
	Open,
	Closed,
}

impl<'a> PreprocessorState<'a> {
	pub fn new<Idlyzer: FnMut(&str) -> FileId + 'a, FLex: FnMut(FileId, &str) -> Option<&'a Vec<(Token, Span)>> + 'a>(
		to_id: Idlyzer,
		lex: FLex,
	) -> PreprocessorState<'a> {
		PreprocessorState {
			definitions: Default::default(),
			errors: vec![],
			state: Default::default(),
			conditional_stack: vec![],
			lex: Box::new(lex),
			to_id: Box::new(to_id),
		}
	}

	// TODO: include file & position
	fn error(&mut self, file_id: FileId, location: Span, msg: String) { self.errors.push(((file_id, location), msg)) }

	pub fn preprocess(&mut self, input: &mut VecDeque<(FileId, Token, Span)>) -> Vec<(FileId, Token, Span)> {
		let mut result: Vec<(FileId, Token, Span)> = vec![];
		let mut previous_file = None;

		while let Some((id, tk, span)) = input.pop_front() {
			if let Some(file) = previous_file {
				if file != id {
					self.state.insert(file, VertexState::Closed);
				}
			}

			match tk {
				Token::PreprocessorDirective(directive) => match directive {
					PreprocessorDirective::Include(_, path) => {
						let recursive_err = format!("Recursive import of {path}");
						let not_found_err = format!("Could not find {path}");

						let file_id = (self.to_id)(&path); // TODO: path resolution here or in to_id
						if self.state.get(&file_id).is_some() {
							self.error(id, span, recursive_err);
						} else if let Some(tokens) = (self.lex)(file_id, &path) as Option<&Vec<(Token, Span)>> {
							self.state.insert(file_id, VertexState::Open);
							input.reserve(input.len() + tokens.len());
							tokens
								.iter()
								.cloned()
								.map(|(tk, span)| (file_id, tk, span))
								.for_each(|x| input.push_front(x));
						} else {
							self.error(id, span, not_found_err);
						}
					}
					ref d @ PreprocessorDirective::If(ref cond) => {
						let c = self.interpret_condition(cond);
						self.conditional_stack.push(((id, d.clone(), span), vec![], c));
						if !c {
							self.skip_conditional_directive(input)
						}
					}
					ref dir @ PreprocessorDirective::ElseIf(ref cond) => {
						if let Some((tk, mut branches, already_processed)) = self.conditional_stack.pop() {
							branches.push((id, dir.clone(), span));
							let include_body = match () {
								_ if already_processed => false, // a previous #if or #elif already matched
								_ if self.interpret_condition(cond) => true,
								_ => false,
							};
							self.conditional_stack.push((tk, branches, already_processed || include_body));
							if !include_body {
								self.skip_conditional_directive(input)
							}
						} else {
							self.error(id, span, "An #elif cannot follow an #else".to_string())
						}
					}
					dir @ PreprocessorDirective::Else => {
						if let Some((tk, mut branches, already_processed)) = self.conditional_stack.pop() {
							if let Some((_, PreprocessorDirective::Else, _)) = branches.last() {
								self.error(id, span.clone(), "This conditional already has an #else".to_string())
							}
							branches.push((id, dir, span));
							self.conditional_stack.push((tk, branches, true));
							if already_processed {
								self.skip_conditional_directive(input)
							}
						} else {
							self.error(id, span, "Dangling #else".to_string())
						}
					}
					PreprocessorDirective::EndIf => {
						if self.conditional_stack.pop().is_none() {
							self.error(id, span, "Dangling #endif".to_string())
						}
					}
					PreprocessorDirective::Define(k, rhs) => {
						self.definitions.insert(k, rhs);
					}
					PreprocessorDirective::Undef(k) => {
						self.definitions.remove(&k);
					}
					PreprocessorDirective::Pragma(_) => todo!(),
					PreprocessorDirective::Other(name, _) => {
						self.error(id, span, format!("Unrecognised directive: {name}"))
					}
				},
				Token::Whitespace => match result.last() {
					// avoid consecutive whitespace in the same file
					Some((existing_id, Token::Whitespace, existing_span)) if *existing_id == id => {
						let combined_span = existing_span.start..span.end;
						result.pop();
						result.push((id, Token::Whitespace, combined_span))
					}
					_ => result.push((id, Token::Whitespace, span)),
				},
				_ => result.push((id, tk, span)),
			}

			previous_file = Some(id);
		}

		self.check_empty_cond_stack();

		result
	}

	/// Skip the body of an `#if`/`#elif` and similar.
	///
	/// Does NOT push to the conditional stack, that's up to the caller, because
	/// the caller typically pops a token before deciding to invoke this method.
	///
	/// Pops a finished conditional from the conditional stack, leaves any
	/// unfinished conditionals there.
	fn skip_conditional_directive(&mut self, input: &mut VecDeque<(FileId, Token, Span)>) {
		let level = self.conditional_stack.len();
		assert!(level > 0);
		// skip till #elif/#else/#endif and process that
		while let Some((id, tk, span)) = input.pop_front() {
			match match tk {
				Token::PreprocessorDirective(d) => d,
				_ => continue,
			} {
				d @ PreprocessorDirective::If(_) => {
					self.conditional_stack.push(((id, d, span), vec![], false));
				}
				d @ PreprocessorDirective::ElseIf(_) if self.conditional_stack.len() == level => {
					// this is an alternative branch to the one we're processing
					// let the top level deal with evaluating the condition
					input.push_front((id, Token::PreprocessorDirective(d), span));
					return;
				}
				PreprocessorDirective::ElseIf(_) =>
				// this is an inner branch we should just skip over
				{
					assert!(self.conditional_stack.len() > level)
				}
				d @ PreprocessorDirective::Else if self.conditional_stack.len() == level => {
					// alternative branch again
					input.push_front((id, Token::PreprocessorDirective(d), span));
					return;
				}
				PreprocessorDirective::Else => assert!(self.conditional_stack.len() > level),
				PreprocessorDirective::EndIf => {
					// dangling endifs are impossible here
					// (we return before processing any extraneous ones)
					self.conditional_stack.pop().unwrap();
					if self.conditional_stack.len() < level {
						return;
					}
				}
				PreprocessorDirective::Undef(_) => (),
				PreprocessorDirective::Pragma(_) => (),
				PreprocessorDirective::Other(_, _) => (),
				PreprocessorDirective::Define(_, _) => (),
				PreprocessorDirective::Include(_, _) => (),
			}
		}

		assert!(
			!self.conditional_stack.is_empty(),
			"should have returned at this point if the input was a well-formed conditional"
		);

		self.check_empty_cond_stack();
		// drain to avoid reporting this error multiple times
		self.conditional_stack.drain((level - 1)..);
	}

	/// Report an error if the conditional stack isn't empty.
	///
	/// Should be called after processing the input stream.
	fn check_empty_cond_stack(&mut self) {
		if let Some(((id, dir, span), _, _)) = self.conditional_stack.last() {
			let name = match dir {
				PreprocessorDirective::If(_) => "if",
				PreprocessorDirective::ElseIf(_) => "elif",
				PreprocessorDirective::Else => "else",
				_ => unreachable!(),
			};
			self.error(*id, span.clone(), format!("This #{name} directive lacks a corresponding #endif"));
		}
	}

	/// Interpret a [PreprocessorExpression] and cast it to a boolean.
	fn interpret_condition(&self, cond: &PreprocessorExpression) -> bool {
		match self.interpret_pp_expr(cond) {
			None => false,
			Some(n) => n != 0,
		}
	}

	fn interpret_pp_expr(&self, cond: &PreprocessorExpression) -> Option<PreprocessorValue> {
		match cond {
			&PreprocessorExpression::IntLiteral(n) => Some(n),
			PreprocessorExpression::Identifier(name) => {
				if let Some(rhs) = self.definitions.get(name) {
					// TODO: avoid reparses?
					let (_, expr) = expression(rhs).ok()?;
					self.interpret_pp_expr(&expr)
				} else {
					None
				}
			}
			PreprocessorExpression::BinOp(op, lhs, rhs) => {
				let l = self.interpret_pp_expr(lhs)?;
				let r = self.interpret_pp_expr(rhs)?;

				Some(match op {
					PreprocessorBinOp::Or => l | r,
					PreprocessorBinOp::Xor => l ^ r,
					PreprocessorBinOp::And => l & r,
					PreprocessorBinOp::Plus => l + r,
					PreprocessorBinOp::Minus => l - r,
					PreprocessorBinOp::Times => l * r,
					PreprocessorBinOp::Divide => l / r,
					PreprocessorBinOp::Modulo => l % r,
					PreprocessorBinOp::Equals => (l == r) as i64,
					PreprocessorBinOp::NotEquals => (l != r) as i64,
					PreprocessorBinOp::LogicalOr => (l != 0 || r != 0) as i64,
					PreprocessorBinOp::LogicalAnd => (l != 0 && r != 0) as i64,
					PreprocessorBinOp::LessThan => (l < r) as i64,
					PreprocessorBinOp::LessOrEqual => (l <= r) as i64,
					PreprocessorBinOp::GreaterThan => (l > r) as i64,
					PreprocessorBinOp::GreaterOrEqual => (l >= r) as i64,
					PreprocessorBinOp::BitwiseShiftLeft => l << r,
					PreprocessorBinOp::BitwiseShiftRight => r >> r,
				})
			}
			PreprocessorExpression::Not(inner) => Some(!self.interpret_condition(inner) as i64),
			PreprocessorExpression::Defined(name) => Some(self.definitions.contains_key(name) as i64),
		}
	}
}

#[cfg(test)]
mod test {
	use crate::{
		base_abstractions::{Buffer, FileId},
		lex,
		lexer::Token,
		Database,
	};

	use super::{parser::*, PreprocessorBinOp as Op, PreprocessorExpression::*, PreprocessorState};
	use pretty_assertions::assert_eq;

	macro_rules! test_pp {
		($str: literal, $vec: expr) => {
			test_pp!($str, $vec, vec![]);
		};
		($str: literal, $vec: expr, $errs: expr) => {{
			let mut errors = vec![];
			assert_eq!(preprocess($str, &mut errors), $vec);
			let expected_errors: Vec<String> = $errs;
			assert_eq!(errors.drain(..).collect::<Vec<String>>(), expected_errors);
		}};
	}

	#[test]
	fn parse_factors() {
		assert_eq!(factor("foo"), Ok(("", Identifier("foo".to_string()))));
		assert_eq!(factor("123 asdf"), Ok(("asdf", IntLiteral(123))));
		assert_eq!(factor("0xff asdf"), Ok(("asdf", IntLiteral(255))));
	}

	#[test]
	fn parse_terms() {
		assert_eq!(exp3("foo"), Ok(("", Identifier("foo".to_string()))));
		assert_eq!(exp3("123 asdf"), Ok(("asdf", IntLiteral(123))));
		assert_eq!(exp3("0xff asdf"), Ok(("asdf", IntLiteral(255))));

		assert_eq!(
			exp3("foo * bar"),
			Ok(("", BinOp(Op::Times, Identifier("foo".to_string()).into(), Identifier("bar".to_string()).into())))
		);
		assert_eq!(
			exp3("12 / asdf"),
			Ok(("", BinOp(Op::Divide, IntLiteral(12).into(), Identifier("asdf".to_string()).into())))
		);
	}

	#[test]
	fn parse_expressions() {
		assert_eq!(
			expression("foo * bar"),
			Ok(("", BinOp(Op::Times, Identifier("foo".to_string()).into(), Identifier("bar".to_string()).into())))
		);
		assert_eq!(
			expression("12 / asdf"),
			Ok(("", BinOp(Op::Divide, IntLiteral(12).into(), Identifier("asdf".to_string()).into())))
		);
		assert_eq!(expression("1 + (2)"), Ok(("", BinOp(Op::Plus, IntLiteral(1).into(), IntLiteral(2).into()))))
	}

	fn preprocess(s: &str, errors: &mut Vec<String>) -> Vec<Token> {
		let db = Database::default();
		let mut pp = PreprocessorState::new(|path| FileId::new(&db, path.into()), |_, _| unreachable!());

		let test_id = FileId::new(&db, "<test-code>.p4".into());
		let input = Buffer::new(&db, s.into());
		let lexed = lex(&db, test_id, input);
		let mut lexemes = lexed.lexemes(&db).iter().cloned().map(|(tk, span)| (test_id, tk, span)).collect();

		let r = pp.preprocess(&mut lexemes).into_iter().map(|(_, tk, _)| tk).collect();

		errors.clear();
		for (_, msg) in pp.errors {
			errors.push(msg)
		}

		r
	}

	#[test]
	fn conditional_inclusion() {
		test_pp!(
			r##"
				#if 1
				foo
				#else
				problem
				#endif
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);

		test_pp!(
			r##"
				#if 0
				problem
				#else
				foo
				#endif
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);

		test_pp!(
			r##"
				#if 0
				problem
				#elif 0
				problem
				#endif
				foo
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);

		test_pp!(
			r##"
				#if 0
				problem
				#elif 1
				foo
				#endif
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);

		test_pp!(
			r##"
				#if 0
				problem
				#elif 0
				problem
				#elif 2 -2
				problem
				#else
				foo
				#endif
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);
	}

	#[test]
	fn complex_conditions() {
		let pp = PreprocessorState::new(|_| unimplemented!(), |_, _| unimplemented!());

		let expr = expression("1 - ( 2 ) + 1").unwrap().1;
		assert_eq!(pp.interpret_pp_expr(&expr), Some(0));

		test_pp!(
			r##"
				#if 3-2*(8-6)+1
				problem
				#else
				foo
				#endif
			"##,
			vec![Token::Whitespace, Token::Identifier("foo".to_string()), Token::Whitespace,]
		);
	}

	#[test]
	fn invalid_input() {
		test_pp!(
			r##"
			#if 1
			foo
			#else
			// missing #endif
			problem
		"##,
			vec![Token::Whitespace, Token::Identifier("foo".into()), Token::Whitespace,],
			vec!["This #if directive lacks a corresponding #endif".to_string(),]
		);

		test_pp!(
			r##"
			#if 0
			problem
			#else
			// missing #endif
			foo
		"##,
			vec![
				Token::Whitespace,
				Token::Comment,
				Token::Whitespace,
				Token::Identifier("foo".into()),
				Token::Whitespace,
			],
			vec!["This #if directive lacks a corresponding #endif".to_string(),]
		);

		test_pp!(
			r##"
			foo
			#else
			// dangling #else
			bar
		"##,
			vec![
				Token::Whitespace,
				Token::Identifier("foo".into()),
				Token::Whitespace,
				Token::Comment,
				Token::Whitespace,
				Token::Identifier("bar".into()),
				Token::Whitespace,
			],
			vec!["Dangling #else".to_string(),]
		);

		test_pp!(
			r##"
			#if 1
			foo
			#else
			nope
			#else
			problem
			#endif
			bar
		"##,
			vec![
				Token::Whitespace,
				Token::Identifier("foo".into()),
				Token::Whitespace,
				Token::Identifier("bar".into()),
				Token::Whitespace,
			],
			vec!["This conditional already has an #else".to_string(),]
		);
	}

	#[test]
	fn defines() {
		test_pp!(
			r##"
			#define x 1
			#if x
			foo
			#else
			problem
			#endif
		"##,
			vec![Token::Whitespace, Token::Identifier("foo".into()), Token::Whitespace,]
		)
	}
}
