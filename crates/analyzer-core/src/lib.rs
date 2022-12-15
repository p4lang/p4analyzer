pub mod preprocessor;

use lazy_static::lazy_static;
use logos::{Logos, Span};
use regex::Regex;

use preprocessor::*;

#[derive(Default)]
#[salsa::db(crate::Jar)]
pub struct Database {
	storage: salsa::Storage<Self>,
}

impl salsa::Database for Database {}

#[salsa::jar(db = Db)]
pub struct Jar(
	Buffer,
	LexedBuffer,
	// gotta include salsa functions as well
	lex,
);

pub trait Db: salsa::DbWithJar<Jar> {}

impl<DB> Db for DB where DB: ?Sized + salsa::DbWithJar<Jar> {}

pub struct Lexer<'db> {
	db: &'db dyn Db,
	pub source: &'db str,
	pub position: usize,
}

/// The input buffer.
#[salsa::input]
pub struct Buffer {
	#[return_ref]
	pub contents: String,
}

#[salsa::tracked]
pub struct LexedBuffer {
	#[return_ref]
	pub lexemes: Vec<(Token, Span)>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub struct Literal {
	pub base: u8,
	pub signed: bool,
	pub width: Option<u32>,
	pub value: i64,
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

	#[regex(r"#\s*\w+", read_directive)]
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
		"include" => {
			// TODO: needs the path
			let (quotation_style, path) = parse_include(buf)?;
			PreprocessorDirective::Include(quotation_style, path)
		}
		"if" => {
			let expr = parse_pp_expression(buf)?;
			PreprocessorDirective::If(expr)
		}
		"elif" => {
			let expr = parse_pp_expression(buf)?;
			PreprocessorDirective::ElseIf(expr)
		}
		"else" => PreprocessorDirective::Else,
		"endif" => PreprocessorDirective::EndIf,
		"define" => {
			let (symbol, rhs) = parse_define(buf)?;
			PreprocessorDirective::Define(symbol, rhs)
		}
		"undef" => {
			let words: Vec<_> = buf.trim().split_ascii_whitespace().collect();
			if words.len() != 1 {
				return None;
			}
			PreprocessorDirective::Undef(words[0].to_string())
		}
		"pragma" => PreprocessorDirective::Pragma(
			buf.chars()
				.skip_while(|ch| ch.is_ascii_whitespace())
				.collect(),
		),
		directive => PreprocessorDirective::Other(directive.to_string(), buf),
	};

	Some(directive)
}

fn parse_include(buf: String) -> Option<(PreprocessorQuotationStyle, String)> {
	use PreprocessorQuotationStyle::*;

	let mut iter = buf.chars().skip_while(|ch| ch.is_ascii_whitespace());
	let quotation_style = match iter.next() {
		Some('<') => Some(AngleBrackets),
		Some('"') => Some(DoubleQuotes),
		_ => None,
	}?;

	let mut buf = String::new();
	loop {
		let ch = iter.next();
		// TODO: escapes?
		match (quotation_style, ch) {
			(AngleBrackets, Some('>')) => break,
			(DoubleQuotes, Some('"')) => break,
			(_, Some(ch)) => buf.push(ch),
			(_, None) => return None,
		};
	}

	Some((quotation_style, buf))
}

fn parse_define(buf: String) -> Option<(String, String)> {
	let mut iter = buf.chars().skip_while(|ch| ch.is_ascii_whitespace());
	todo!()
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
pub fn lex(db: &dyn crate::Db, buf: Buffer) -> LexedBuffer {
	let contents = buf.contents(db);
	let lexer = Token::lexer(contents);
	// TODO: we should extract the slices or at least spans as well
	LexedBuffer::new(db, lexer.spanned().collect())
}
