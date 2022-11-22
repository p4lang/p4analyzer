use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use logos::Logos;
use regex::Regex;
use salsa::DebugWithDb;

#[derive(Default)]
#[salsa::db(crate::Jar)]
pub(crate) struct Database {
	storage: salsa::Storage<Self>,
	logs: Option<Arc<Mutex<Vec<String>>>>,
}

impl salsa::Database for Database {
	fn salsa_event(&self, event: salsa::Event) {
		// Log interesting events, if logging is enabled
		if let Some(logs) = &self.logs {
			// don't log boring events
			if let salsa::EventKind::WillExecute { .. } = event.kind {
				logs.lock()
					.unwrap()
					.push(format!("Event: {:?}", event.debug(self)));
			}
		}
	}
}

#[salsa::jar(db = Db)]
pub struct Jar(
	Buffer,
	LexedBuffer,
	// gotta include salsa functions as well
	lex,
);

pub trait Db: salsa::DbWithJar<Jar> {}

impl<DB> Db for DB where DB: ?Sized + salsa::DbWithJar<Jar> {}

/// The input buffer.
#[salsa::input]
pub struct Buffer {
	#[return_ref]
	pub contents: String,
}

#[salsa::tracked]
pub struct LexedBuffer {
	#[return_ref]
	pub lexemes: Vec<Token>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub struct Literal {
	base: u8,
	signed: bool,
	width: Option<u32>,
	value: i64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorDirective {
	Include,
	Other(String),
}

#[derive(Logos, PartialOrd, Ord, PartialEq, Eq, Debug, Clone)]
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
	#[regex("/*", read_comment)]
	Comment,

	#[token(".")]
	Dot,

	#[token("=")]
	Equals,

	#[error]
	Error,

	#[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
	Identifier,

	/// An integer literal.
	///
	/// See [the P4 specification](https://p4.org/p4-spec/docs/P4-16-v-1.2.3.html#sec-integer-literals).
	/// There's [a regular expression](https://regex101.com/r/A79tJL/1) that should match the specification exactly,
	/// but this rule uses a relaxed one instead.
	#[regex("[0-9][0-9wsbBdDoOxXa-fA-F_]*", read_int)]
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

	#[regex(r"#\s*\w.*", read_directive)]
	PreprocessorDirective(PreprocessorDirective),

	#[token(";")]
	Semicolon,

	#[regex(r"\s\s*")]
	Whitespace,
}

fn read_directive(lex: &mut logos::Lexer<Token>) -> Option<PreprocessorDirective> {
	lazy_static! {
		static ref DIRECTIVE: Regex = Regex::new(r"#\s*(\w+)").unwrap();
	}

	let str = lex.slice();
	let caps = DIRECTIVE.captures(str)?;

	let mut rem = lex.remainder().char_indices();
	let mut buf = String::new();
	let mut escape = false;

	loop {
		match rem.next() {
			Some((_, '\\')) if !escape => escape = true,
			Some((offset, '\n')) if !escape => {
				lex.bump(offset);
				break;
			}
			// TODO: any real escapes?
			Some((_, char)) => {
				buf.push(char);
				escape = false
			}
			None => {
				lex.bump(lex.remainder().len());
				break;
			}
		};
	}

	let directive = match caps.get(1)?.as_str() {
		"include" => PreprocessorDirective::Include, // TODO: needs the path
		_ => PreprocessorDirective::Other(buf),
	};

	Some(directive)
}

// TODO: report nice errors
fn read_int(lex: &mut logos::Lexer<Token>) -> Option<Literal> {
	let mut lit = Literal {
		base: 10,
		signed: false,
		width: None,
		value: 0,
	};

	let str = lex.slice();
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

	lit.value = i64::from_str_radix(str, lit.base as u32).ok()?;
	Some(lit)
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

fn read_comment(lex: &mut logos::Lexer<Token>) -> bool {
	let mut rem = lex.remainder().char_indices();
	let mut asterisk = false;

	loop {
		match rem.next() {
			Some((_, '*')) => asterisk = true,
			Some((_, '/')) if asterisk => break,
			Some(_) => asterisk = false,
			None => break,
		};
	}

	true
}

#[salsa::tracked(return_ref)]
fn lex(db: &dyn crate::Db, buf: Buffer) -> LexedBuffer {
	let contents = buf.contents(db);
	let lexer = Token::lexer(contents);
	// TODO: we should extract the slices or at least spans as well
	LexedBuffer::new(db, lexer.collect())
}

#[cfg(test)]
mod tests {
	use super::*;
	use pretty_assertions::{assert_eq, assert_ne};

	fn lex_str(s: &str) -> Vec<Token> {
		let db = Database::default();
		let buf = Buffer::new(&db, s.to_string());
		let lexed = lex(&db, buf);
		lexed.lexemes(&db).clone()
	}

	#[test]
	fn it_works() {
		use Token::Identifier;
		assert_eq!(lex_str("hello"), vec![Identifier]);
	}

	#[test]
	fn int_literals() {
		use Token::{Integer, Whitespace};
		assert_eq!(
			lex_str(
				r##"
				123
				10s5
				2w11
				0xff
			"##
			),
			vec![
				Whitespace,
				Integer(Literal {
					base: 10,
					signed: false,
					width: None,
					value: 123
				}),
				Whitespace,
				Integer(Literal {
					base: 10,
					signed: true,
					width: Some(10),
					value: 5
				}),
				Whitespace,
				Integer(Literal {
					base: 10,
					signed: false,
					width: Some(2),
					value: 11
				}),
				Whitespace,
				Integer(Literal {
					base: 16,
					signed: false,
					width: None,
					value: 255
				}),
				Whitespace,
			]
		);
	}

	#[test]
	fn real_p4() {
		use self::PreprocessorDirective::*;
		use Token::*;

		assert_eq!(
			lex_str(
				r##"
			// Include P4 core library
			# include <core.p4>

			// Include very simple switch architecture declarations
			# include "very_simple_switch_model.p4"
			# foo something

			// This program processes packets comprising an Ethernet and an IPv4
			// header, and it forwards packets using the destination IP address

			typedef bit<48>  EthernetAddress;
			typedef bit<32>  IPv4Address;

			// Standard Ethernet header
			header Ethernet_h {
				EthernetAddress dstAddr;
				EthernetAddress srcAddr;
				bit<16>         etherType;
			}
			"##
			), vec![
				Whitespace,
				Comment,
				Whitespace,
				PreprocessorDirective(Include),
				Whitespace,
				Comment,
				Whitespace,
				PreprocessorDirective(Include),
				Whitespace,
				Comment,
				Whitespace,
				Comment,
				Whitespace,
				KwTypedef,
				Whitespace,
				Identifier,
				OpenChevron,
				Integer(Literal {
					base: 10,
					signed: false,
					width: None,
					value: 48
				}),
				CloseChevron,
				Whitespace,
				Identifier,
				Semicolon,
				Whitespace,
				KwTypedef,
				Whitespace,
				Identifier,
				OpenChevron,
				Integer(Literal {
					base: 10,
					signed: false,
					width: None,
					value: 32
				}),
				CloseChevron,
				Whitespace,
				Identifier,
				Semicolon,
				Whitespace,
				Comment,
				Whitespace,
				KwHeader,
				Whitespace,
				Identifier,
				Whitespace,
				OpenBrace,
				Whitespace,
				Identifier,
				Whitespace,
				Identifier,
				Semicolon,
				Whitespace,
				Identifier,
				Whitespace,
				Identifier,
				Semicolon,
				Whitespace,
				Identifier,
				OpenChevron,
				Integer(Literal {
					base: 10,
					signed: false,
					width: None,
					value: 16
				}),
				CloseChevron,
				Whitespace,
				Identifier,
				Semicolon,
				Whitespace,
				CloseBrace,
				Whitespace,
			]
		);
	}
}
