#![allow(clippy::tabs_in_doc_comments)]
use lazy_static::lazy_static;
use logos::{Logos, Span};
use regex::Regex;

use super::{base_abstractions::*, preprocessor::*};

#[salsa::tracked]
pub struct LexedBuffer {
	#[return_ref]
	pub lexemes: Vec<(Token, Span)>,
}

pub struct Lextras {
	pub db: Option<*const dyn crate::Db>,
	pub file_id: FileId,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Hash)]
pub struct Literal {
	pub base: u8,
	pub signed: bool,
	pub width: Option<u32>,
	pub value: i64,
}

#[derive(Logos, PartialOrd, Ord, PartialEq, Eq, Debug, Clone, Hash)]
#[logos(extras = Lextras)]
pub enum Token {
	#[token("}")]
	CloseBrace,

	#[token("]")]
	CloseBracket,

	#[token(">")]
	CloseChevron,

	#[token(")")]
	CloseParen,

	#[token(":")]
	Colon,

	#[token(",")]
	Comma,

	#[regex("//[^\n]*\n")]
	#[regex("/\\*", |lex| Lexer(lex).read_comment())]
	Comment,

	#[token(".")]
	Dot,

	#[token("=")]
	Equals,

	#[token("*")]
	Asterisk,

	#[token("/")]
	Slash,

	#[token("+")]
	Plus,

	#[token("-")]
	Minus,

	#[error]
	Error,

	#[regex("[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
	Identifier(String),

	/// An integer literal.
	///
	/// See [the P4 specification](https://p4.org/p4-spec/docs/P4-16-v-1.2.3.html#sec-integer-literals).
	/// There's [a regular expression](https://regex101.com/r/A79tJL/1) that should match the specification exactly,
	/// but this rule uses a relaxed one instead.
	#[regex("[0-9][0-9wsbBdDoOxXa-fA-F_]*", |lex| Lexer(lex).read_int())]
	Integer(Literal),

	#[token("action")]
	KwAction,

	#[token("apply")]
	KwApply,

	#[token("control")]
	KwControl,

	#[token("else")]
	KwElse,

	#[token("header")]
	KwHeader,

	#[token("if")]
	KwIf,

	#[token("parser")]
	KwParser,

	#[token("return")]
	KwReturn,

	#[token("state")]
	KwState,

	#[token("table")]
	KwTable,

	#[token("transition")]
	KwTransition,

	#[token("typedef")]
	KwTypedef,

	#[token("{")]
	OpenBrace,

	#[token("[")]
	OpenBracket,

	#[token("<")]
	OpenChevron,

	#[token("(")]
	OpenParen,

	#[regex(r"#\s*\w+", |lex| Lexer(lex).read_directive())]
	PreprocessorDirective(PreprocessorDirective),

	#[token(";")]
	Semicolon,

	#[token("@")]
	AtSymbol,

	#[regex(r"\s\s*")]
	Whitespace,
}

struct Lexer<'a, 'b>(&'b mut logos::Lexer<'a, Token>);

impl<'a, 'b> Lexer<'a, 'b> {
	// TODO: report nice errors
	fn read_int(mut self) -> Option<Literal> {
		let mut lit = Literal { base: 10, signed: false, width: None, value: 0 };

		let str = self.0.slice();
		// the stitching here is a little ugly, fn(&str) -> Option<(T, &str)> here
		// is trying to be something like a state monad.
		let str = parse_width(str)
			.map(|((width, sign), str)| {
				lit.width = Some(width);
				lit.signed = sign;
				str
			})
			.unwrap_or(str);
		let str = parse_base(str)
			.map(|(base, str)| {
				lit.base = base;
				str
			})
			.unwrap_or(str);

		lit.value = match i64::from_str_radix(str, lit.base as u32).ok() {
			Some(i) => i,
			None => {
				self.report(Severity::Error, "malformed integer literal");
				return None;
			}
		};
		Some(lit)
	}

	fn read_directive(mut self) -> Option<PreprocessorDirective> {
		lazy_static! {
			static ref DIRECTIVE: Regex = Regex::new(r"#\s*(\w+)").unwrap();
		}

		let str = self.0.slice();
		let caps = DIRECTIVE.captures(str)?;
		let arg = self.read_line_with_newline_escapes(self.0.remainder());

		let directive = match caps.get(1)?.as_str() {
			"include" => {
				let (quotation_style, path) = self.parse_include(arg)?;
				PreprocessorDirective::Include(quotation_style, path)
			}
			"if" => {
				let expr = parse_pp_expression(arg)?;
				PreprocessorDirective::If(expr)
			}
			"elif" => {
				let expr = parse_pp_expression(arg)?;
				PreprocessorDirective::ElseIf(expr)
			}
			"else" => PreprocessorDirective::Else,
			"endif" => PreprocessorDirective::EndIf,
			"define" => {
				let (symbol, rhs) = self.parse_define(arg)?;
				PreprocessorDirective::Define(symbol, rhs)
			}
			"undef" => {
				let words: Vec<_> = arg.trim().split_ascii_whitespace().collect();
				if words.len() != 1 {
					return None;
				}
				PreprocessorDirective::Undef(words[0].to_string())
			}
			"pragma" => PreprocessorDirective::Pragma(arg.chars().skip_while(|ch| ch.is_ascii_whitespace()).collect()),
			directive => PreprocessorDirective::Other(directive.to_string(), arg),
		};

		Some(directive)
	}

	fn parse_include(&mut self, buf: String) -> Option<(PreprocessorQuotationStyle, String)> {
		use PreprocessorQuotationStyle::*;

		if let Some(ch) = buf.chars().next() {
			if !ch.is_ascii_whitespace() {
				self.report(Severity::Error, "the include directive and its argument must be separated by whitespace")
			}
		}

		let mut iter = buf.chars().skip_while(|ch| ch.is_ascii_whitespace());
		let quotation_style = match iter.next() {
			Some('<') => Some(AngleBrackets),
			Some('"') => Some(DoubleQuotes),
			_ => {
				self.report(
					Severity::Error,
					"include path must start with an opening angle bracket ('<') or double quote ('\"')",
				);
				None
			}
		}?;

		let terminator = match quotation_style {
			AngleBrackets => '>',
			DoubleQuotes => '"',
		};

		let mut buf = String::new();
		loop {
			let ch = iter.next();
			// TODO: escapes?
			match ch {
				Some(ch) if ch == terminator => break,
				Some(ch) => buf.push(ch),
				None => {
					self.report(Severity::Error, &format!("include path must end with a '{terminator}'"));
					return None;
				}
			};
		}

		Some((quotation_style, buf))
	}

	fn parse_define(&mut self, buf: String) -> Option<(String, String)> {
		let iter = buf.chars().skip_while(|ch| ch.is_ascii_whitespace());

		let mut iter = iter.peekable();
		if let Some(n) = iter.peek() {
			if !n.is_ascii_alphabetic() && *n != '_' {
				self.report(Severity::Error, "the name of a macro must start with a letter or an underscore ('_')");
				return None;
			}
		}

		let mut symbol = String::new();
		while let Some(ch) = iter.peek() {
			if ch.is_ascii_alphanumeric() || *ch == '_' {
				symbol.push(*ch);
				iter.next();
			} else {
				break;
			}
		}

		// skip whitespace between identifier and rhs
		while let Some(ch) = iter.peek() {
			if !ch.is_ascii_whitespace() {
				break;
			}
			iter.next();
		}

		let mut escape = false;
		let mut rhs = String::new();
		loop {
			let ch = iter.next();

			// FIXME: these escapes do not work for string literals
			match (escape, ch) {
				(true, Some(ch)) => rhs.push(ch),
				(_, Some('\\')) => escape = true,
				(_, Some('\n')) => break,
				(_, Some(ch)) => rhs.push(ch),
				(true, None) => todo!("unfinished macro"),
				(false, None) => break,
			}
		}

		Some((symbol, rhs))
	}

	fn read_comment(&mut self) -> bool {
		let mut rem = self.0.remainder().chars();
		let mut asterisk = false;

		loop {
			match rem.next().map(|ch| {
				self.0.bump(ch.len_utf8());
				ch
			}) {
				Some('*') => asterisk = true,
				Some('/') if asterisk => break true,
				Some(_) => asterisk = false,
				None => break false,
			};
		}
	}

	/** Read characters until and excluding the next newline, processing newline escapes.
	 * Escaped non-newline characters (e.g. "\k") are added to the result verbatim, but such backslashes are not.
	 */
	fn read_line_with_newline_escapes(&mut self, s: &str) -> String {
		let mut iter = s.chars();
		let mut buf = String::new();
		let mut escape = false;
		loop {
			match iter.next().map(|ch| {
				self.0.bump(ch.len_utf8());
				ch
			}) {
				Some('\\') if !escape => escape = true,
				Some('\n') if !escape => break,
				Some(char) => {
					buf.push(char);
					escape = false
				}
				None => break,
			}
		}
		buf
	}

	fn report(&mut self, severity: Severity, msg: &str) {
		if let Some(db) = self.0.extras.db.map(|db| unsafe { &*db }) {
			Diagnostics::push(
				db,
				Diagnostic { file: self.0.extras.file_id, location: self.0.span(), severity, message: msg.to_string() },
			);
		}
	}
}

fn parse_base(str: &str) -> Option<(u8, &str)> {
	lazy_static! {
		static ref BASE: Regex = Regex::new("0([bBdDoOxX])").unwrap();
	}

	BASE.captures(str).map(|caps| {
		let base_indicator = caps.get(1).unwrap();
		let base = match base_indicator.as_str() {
			"b" | "B" => 2,
			"d" | "D" => 10,
			"o" | "O" => 8,
			"x" | "X" => 16,
			_ => unreachable!(),
		};
		(base, &str[base_indicator.end()..])
	})
}

fn parse_width(str: &str) -> Option<((u32, bool), &str)> {
	lazy_static! {
		static ref WIDTH: Regex = Regex::new("([0-9][0-9]*)([ws])").unwrap();
	}

	WIDTH.captures(str).map(|caps| {
		let width = caps.get(1).unwrap();
		let sign = caps.get(2).unwrap();

		let width = width.as_str().parse().unwrap();
		let signed = match sign.as_str() {
			"w" => false,
			"s" => true,
			_ => unreachable!(),
		};
		((width, signed), &str[sign.end()..])
	})
}
