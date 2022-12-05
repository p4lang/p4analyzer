use super::*;
use pretty_assertions::assert_eq;

fn lex_str(s: &str) -> Vec<Token> {
	let db = Database::default();
	let buf = Buffer::new(&db, s.to_string());
	let lexed = lex(&db, buf);
	lexed
		.lexemes(&db)
		.iter()
		.map(|(tk, _)| tk)
		.cloned()
		.collect()
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
			PreprocessorDirective(Include(
				DoubleQuotes,
				"very_simple_switch_model.p4".to_string()
			)),
			Whitespace,
			PreprocessorDirective(Other("foo".to_string(), " something".to_string())),
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
