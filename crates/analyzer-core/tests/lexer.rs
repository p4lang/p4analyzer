extern crate analyzer_core;

use analyzer_core::{lsp_position::LspFile, *};
use base_abstractions::*;
use lexer::*;
use pretty_assertions::assert_eq;

fn lex_str(s: &str) -> Vec<Token> {
	let db = Database::new(|base, _| Ok(base.into()));
	let buf = Buffer::new(&db, s.to_string(), LspFile::new(&s.to_string()));
	let file_id = FileId::new(&db, "foo.p4".to_string());
	let lexed = lex(&db, file_id, buf);
	lexed.lexemes(&db).iter().map(|(tk, _)| tk).cloned().collect()
}

#[test]
fn it_works() {
	use Token::Identifier;
	assert_eq!(lex_str("hello"), vec![Identifier("hello".to_string())]);
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
			Integer(Literal { base: 10, signed: false, width: None, value: 123 }),
			Whitespace,
			Integer(Literal { base: 10, signed: true, width: Some(10), value: 5 }),
			Whitespace,
			Integer(Literal { base: 10, signed: false, width: Some(2), value: 11 }),
			Whitespace,
			Integer(Literal { base: 16, signed: false, width: None, value: 255 }),
			Whitespace,
		]
	);
}

#[test]
fn real_p4() {
	use preprocessor::{PreprocessorDirective::*, *};
	use PreprocessorQuotationStyle::*;
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
		),
		vec![
			Whitespace,
			Comment,
			Whitespace,
			PreprocessorDirective(Include(AngleBrackets, "core.p4".to_string())),
			Whitespace,
			Comment,
			Whitespace,
			PreprocessorDirective(Include(DoubleQuotes, "very_simple_switch_model.p4".to_string())),
			Whitespace,
			PreprocessorDirective(Other("foo".to_string(), " something".to_string())),
			Whitespace,
			Comment,
			Whitespace,
			Comment,
			Whitespace,
			KwTypedef,
			Whitespace,
			Identifier("bit".to_string()),
			OpenChevron,
			Integer(Literal { base: 10, signed: false, width: None, value: 48 }),
			CloseChevron,
			Whitespace,
			Identifier("EthernetAddress".to_string()),
			Semicolon,
			Whitespace,
			KwTypedef,
			Whitespace,
			Identifier("bit".to_string()),
			OpenChevron,
			Integer(Literal { base: 10, signed: false, width: None, value: 32 }),
			CloseChevron,
			Whitespace,
			Identifier("IPv4Address".to_string()),
			Semicolon,
			Whitespace,
			Comment,
			Whitespace,
			KwHeader,
			Whitespace,
			Identifier("Ethernet_h".to_string()),
			Whitespace,
			OpenBrace,
			Whitespace,
			Identifier("EthernetAddress".to_string()),
			Whitespace,
			Identifier("dstAddr".to_string()),
			Semicolon,
			Whitespace,
			Identifier("EthernetAddress".to_string()),
			Whitespace,
			Identifier("srcAddr".to_string()),
			Semicolon,
			Whitespace,
			Identifier("bit".to_string()),
			OpenChevron,
			Integer(Literal { base: 10, signed: false, width: None, value: 16 }),
			CloseChevron,
			Whitespace,
			Identifier("etherType".to_string()),
			Semicolon,
			Whitespace,
			CloseBrace,
			Whitespace,
		]
	);
}

#[test]
fn long_comment() {
	use Token::*;

	assert_eq!(
		lex_str(
			r#"
			identifier
			/* poof */
		"#
		),
		vec![Whitespace, Identifier("identifier".into()), Whitespace, Comment, Whitespace,]
	);

	assert_eq!(
		lex_str(
			r#"
			/* if you're happy and you know it sign your commits with GPG */
			/*//* incomplete long *comment*
		"#
		),
		vec![Whitespace, Comment, Whitespace, Error,]
	);
}

#[test]
fn unknown_directive() {
	use Token::*;
	assert_eq!(
		lex_str(
			r#"
	#defineoops 1
	"#
		),
		vec![
			Whitespace,
			PreprocessorDirective(preprocessor::PreprocessorDirective::Other("defineoops".into(), " 1".into())),
			Whitespace,
		]
	);
}

#[test]
fn preprocessor_parser() {
	use preprocessor::*;
	use PreprocessorBinOp::*;
	use PreprocessorExpression::*;

	assert_eq!(
		parse_pp_expression("2 == 3".to_string()),
		Some(BinOp(Equals, Box::new(IntLiteral(2)), Box::new(IntLiteral(3))))
	);
}

#[test]
fn includes() {
	let foo = lex_str(
		r##"
		#include <bar.p4>
		foo 321
	"##,
	);
	let bar = lex_str(
		r##"
		bar 123
	"##,
	);
}
