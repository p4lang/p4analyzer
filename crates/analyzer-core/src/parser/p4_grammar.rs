
use anyhow::Result;
use lazy_static::lazy_static;
use parking_lot::RwLock;

use super::*;
use crate::lexer::Token;

lazy_static! {
	static ref STRING_TO_TOKEN: HashMap<&'static str, Token> = ([
		("*", Token::Asterisk),
		("@", Token::AtSymbol),
		(",", Token::Comma),
		("(", Token::OpenParen),
		(")", Token::CloseParen),
	]).into();
}

macro_rules! rule_rhs {
	($lit:literal) => {
		{
			let lit: &'static str = $lit;
			// TODO: keep Arc's in the table
			Rule::Terminal(Rc::new(vec![STRING_TO_TOKEN[lit].clone()]))
		}
	};
	($name:ident | $($names:ident)|+) => {
		Rule::Choice(vec![stringify!($name), $(stringify!($names)),+])
	};
	($name:ident, $($names:ident),+) => {
		Rule::Sequence(vec![stringify!($name), $(stringify!($names)),+])
	};
	($name:ident rep) => {
		Rule::Repetition(stringify!($name))
	};
	($name:ident) => {
		Rule::Sequence(vec![stringify!($name)])
	};
	((Token::$name:ident)) => {
		// TODO: keep Rc's in a lookup table
		Rule::Terminal(Rc::new(vec![Token::$name]))
	};
	(()) => {
		Rule::Nothing
	};
	({$pat:pat $(if $cond:expr)?}) => {
		Rule::TerminalPredicate(|tk| match tk {
			$pat $(if $cond)? => true,
			_ => false,
		}, stringify!($pat $(if $cond:expr)?))
	};
}

macro_rules! grammar {
	($($name:ident =>
		$prefix:tt
		$(| $($or:tt)|+)?
		$(, $($seq:tt),+)?
		$($rep:ident)?
	);+$(;)?) => {
		[$((stringify!($name), rule_rhs!($prefix $(| $($or)|+)? $(, $($seq),+)? $($rep)?))),+]
	};
}

pub fn p4_parser() -> impl FnOnce(RwLock<Vec<Token>>) -> Parser<Token> {
	let rules = grammar! {
		start => p4program;
		ws => whitespace rep;
		whitespace => (Token::Whitespace);

		p4program => ws, top_level_decls, ws;
		top_level_decls => top_level_decls_rep | top_level_decls_end | nothing;
		top_level_decls_rep => top_level_decl, ws, top_level_decls;
		top_level_decls_end => (Token::Semicolon);

		top_level_decl => parser_decl;
		annotations => annotation rep;
		annotation => at_symbol, ident;

		direction => dir_in | dir_out | dir_inout;
		dir_in    => { Token::Identifier(i) if i == "in" };
		dir_out   => { Token::Identifier(i) if i == "out" };
		dir_inout => { Token::Identifier(i) if i == "inout" };

		at_symbol => "@";
		comma => ",";
		close_paren => ")";
		open_paren => "(";
		ident => { Token::Identifier(_) };
		nothing => ();

		parser_kw => (Token::KwParser);
		parser_decl => annotations, ws, parser_kw, ws, ident, ws, opt_type_params, ws, parameter_list;

		parameter_list => open_paren, ws, parameter_seq, ws, close_paren;
		parameter_seq => parameter_seq_rep | parameter | nothing;
		parameter_seq_rep => parameter_comma, parameter_seq;
		parameter_comma => parameter, ws, comma;
		maybe_comma => comma | nothing;
		parameter => maybe_annotation, ws, maybe_direction, ws, typ, ws, ident;
		maybe_annotation => annotation | nothing;
		maybe_direction => direction | nothing;
		opt_type_params => nothing; // TODO: type params
		typ => ident; // TODO: full type syntax
	};

	Parser::from_rules(&rules).unwrap()
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic() -> Result<()> {
		let mk_parser = p4_parser();
		let source = vec![
			Token::Whitespace,
			Token::KwParser,
			Token::Identifier("()".into()),
			Token::OpenParen,
			Token::CloseParen,
		];
		let source_lock = RwLock::new(source);
		let mut parser: Parser<Token> = mk_parser(source_lock);

		let r = parser._match();
		eprint!("here it is {r:#?}");
		assert_eq!(r, Ok(Cst::Repetition(vec![])));

		Ok(())
	}
}
