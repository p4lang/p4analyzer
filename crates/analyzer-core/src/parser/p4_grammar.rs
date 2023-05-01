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
		(";", Token::Semicolon),
		("(", Token::OpenParen),
		(")", Token::CloseParen),
		("{", Token::OpenBrace),
		("}", Token::CloseBrace),
		("[", Token::OpenBracket),
		("]", Token::CloseBracket),
		("=", Token::Equals),
		(":", Token::Colon),
		(".", Token::Dot),
		("*", Token::Asterisk),
		("/", Token::Slash),
		("+", Token::Plus),
		("-", Token::Minus),
		(">", Token::CloseChevron),
		("<", Token::OpenChevron),
		("?", Token::QuestionMark),
		("!", Token::ExclamationMark),
		("&", Token::Ampersand),
		("|", Token::Pipe),
		("^", Token::Caret),
		("%", Token::Percent),
		("~", Token::Tilde),
	])
	.into();
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
		Rule::Choice(vec![$name, $($names),+])
	};
	($name:ident, $($names:ident),+) => {
		Rule::Sequence(vec![$name, $($names),+])
	};
	($name:ident rep) => {
		Rule::Repetition($name)
	};
	($name:ident) => {
		Rule::Sequence(vec![$name])
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

macro_rules! grammar_trivia {
	() => {
		Default::default()
	};
	($(@$annotation:ident $($annotated_rule:ident)+;)*) => {
		{
			let mut trivia = BTreeMap::<P4GrammarRules, TriviaClass>::new();
			$(
				{
					let class: TriviaClass = TriviaClass::$annotation;
					$(
						trivia.insert($annotated_rule, class);
					)+
				}
			)*
			trivia
		}
	};
}

macro_rules! grammar_rules {
	($($name:ident =>
		$prefix:tt
		$(| $($or:tt)|+)?
		$(, $($seq:tt),+)?
		$($rep:ident)?
	);+$(;)?) => {
		[$(($name, rule_rhs!($prefix $(| $($or)|+)? $(, $($seq),+)? $($rep)?))),+]
	};
}

macro_rules! grammar {
	(
		$(@$annotation:ident $($annotated_rule:ident)+;)*
		$($(#[doc = $docs:tt])* $name:ident =>
			$prefix:tt
			$(| $($or:tt)|+)?
			$(, $($seq:tt),+)?
			$($rep:ident)?
		);+$(;)?
	) => {
		#[allow(non_camel_case_types, unused)]
		#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
		pub enum P4GrammarRules {
			$($(#[doc = $docs])* $name),+
		}

		pub fn get_grammar() -> Grammar<P4GrammarRules, Token> {
			use P4GrammarRules::*;

			Grammar {
				initial: start,
				rules: grammar_rules!($($name =>
						$prefix
						$(| $($or)|+)?
						$(, $($seq),+)?
						$($rep)?
					);+).into(),
				trivia: grammar_trivia!($(@$annotation $($annotated_rule)+;)*),
			}
		}
	};
}

// TODO: lazy_static!
grammar! {
	@SkipNodeAndChildren
		at_symbol
		close_paren
		comma
		semicolon
		nothing
		open_paren
		parser_kw
		ws
	;

	@SkipNodeOnly
		maybe_direction
		parameter_comma
		parameter_seq
		parameter_seq_rep
		top_level_decls_rep
	;

	start => p4program;
	ws => whitespace rep;
	whitespace => (Token::Whitespace);

	p4program => ws, top_level_decls, ws;
	top_level_decls => top_level_decls_rep | top_level_decls_end | nothing;
	top_level_decls_rep => top_level_decl, ws, maybe_semicolon, ws, top_level_decls;
	top_level_decls_end => semicolon;
	maybe_semicolon => semicolon | nothing;

	top_level_decl => parser_decl;
	annotations => annotation rep;
	annotation => ws, at_symbol, ident;

	direction => dir_in | dir_out | dir_inout;
	dir_in    => { Token::Identifier(i) if i == "in" };
	dir_out   => { Token::Identifier(i) if i == "out" };
	dir_inout => { Token::Identifier(i) if i == "inout" };

	/// Semantic non-terminal that marks an identifier as a definition.
	///
	/// For example, in `parser MyParser<T>(inout T x) { }`, `MyParser`, `T`,
	/// and `x` are all definitions, and possible targets for go-to definition.
	definition => ident;
	ident => { Token::Identifier(_) };
	number => { Token::Integer(_) };
	semicolon => ";";
	at_symbol => "@";
	comma => ",";
	close_paren => ")";
	open_paren => "(";
	nothing => ();
	open_brace => "{";
	close_brace => "}";
	equals => "=";
	plus => "+";
	minus => "-";
	star => "*";
	slash => "/";
	percent => "%";
	ampersand => "&";
	pipe => "|";
	exclamation => "!";
	lt => "<";
	gt => ">";
	lte => lt, equals;
	gte => gt, equals;
	eq => equals, equals;
	neq => exclamation, equals;
	and => ampersand, ampersand;
	or => pipe, pipe;

	bin_op => plus | minus | star | slash | percent | lt | gt | lte | gte | eq | neq | and | or;

	parser_kw => (Token::KwParser);
	parser_decl => ws, annotations, ws, parser_kw, ws, definition, ws, maybe_type_params, ws, parameter_list, ws, maybe_block;

	parameter_list => open_paren, ws, parameter_seq, ws, close_paren;
	parameter_seq => parameter_seq_rep | parameter | nothing;
	parameter_seq_rep => parameter_comma, parameter_seq;
	parameter_comma => ws, parameter, ws, comma;
	maybe_comma => comma | nothing;
	parameter => annotations, ws, maybe_direction, ws, typ, ws, definition;
	maybe_direction => direction | nothing;
	maybe_type_params => nothing; // TODO: type params
	typ => ident; // TODO: full type syntax

	maybe_block => block | nothing;
	block => open_brace, ws, statements, ws, close_brace;
	statements => statement rep;
	statement => ws, stmt, ws, maybe_semicolon, ws;
	stmt => definition_stmt | assignment_stmt | top_level_decl;
	definition_stmt => annotations, ws, typ, ws, definition, ws, maybe_definition;
	maybe_definition => equals, ws, rhs;
	assignment_stmt => lhs, ws, equals, ws, rhs;
	lhs => ident;
	rhs => expression;

	expression => ident | number | paren_expr | bin_op_expr;
	bin_op_expr => expression, ws, bin_op, ws, expression;
	paren_expr => open_paren, ws, expression, ws, close_paren;
}

pub fn p4_parser() -> impl FnOnce(RwLock<Vec<Token>>) -> Parser<P4GrammarRules, Token> {
	Parser::from_grammar(get_grammar()).unwrap()
}

#[cfg(test)]
mod test {
	use super::{super::ast::*, *};
	use pretty_assertions::{assert_eq, assert_ne};

	fn lex_str(s: &str) -> Vec<Token> {
		use crate::*;

		let db = Database::default();
		let buf = Buffer::new(&db, s.to_string());
		let file_id = FileId::new(&db, "foo.p4".to_string());
		let lexed = lex(&db, file_id, buf);
		lexed.lexemes(&db).iter().map(|(tk, _)| tk).cloned().collect()
	}

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
		let mut parser: Parser<P4GrammarRules, Token> = mk_parser(source_lock);

		let (_, _, r) = parser.parse();
		eprintln!("here it is {r:#?}");
		assert_eq!(r, Ok(ExistingMatch { cst: Cst::Repetition(vec![]), match_length: 0 }));

		Ok(())
	}

	#[test]
	fn with_lexer() -> Result<()> {
		let mk_parser = p4_parser();
		let stream = lex_str(
			r"
			parser test_parser(@annotation in type int_param, short short_param);
		",
		);

		let source_lock = RwLock::new(stream);
		let mut parser = mk_parser(source_lock);

		let (_, _, parsed) = parser.parse();
		// assert_eq!(Err(ParserError::ExpectedEof), parsed);

		let syntax_node = SyntaxNode::new_root(parser.grammar.clone(), super::ast::GreenNode(Rc::new(parsed.unwrap())));
		println!("I am {:?}", syntax_node.kind());

		for (depth, child) in preorder(0, syntax_node) {
			println!("{}- {:?}", "  ".repeat(depth as usize), child.kind());
			if let Some(parser) = ParserDecl::cast(child) {
				println!("parser declaration with params");
				for param in parser.parameter_list().next().unwrap().parameter() {
					let p = param.definition().flat_map(|d| d.ident()).next().unwrap();
					let d = param
						.direction()
						.next()
						.map(|d| d.variant.to_string())
						.unwrap_or("<no direction>".to_string());

					println!("  {d} {} at token {}", p.as_str(), p.offset());
				}
			}
		}

		// assert_eq!(
		// 	simplify((*parser.grammar).clone(), parsed.unwrap()),
		// 	P4Program {
		// 		top_level_declarations: vec![TopLevelDeclaration {
		// 			annotations: vec![],
		// 			kind: TopLevelDeclarationKind::Parser(ParserDeclaration {
		// 				parameters: ParameterList {
		// 					list: vec![Parameter {
		// 						annotations: vec![Annotation::Unknown("annotation".into())],
		// 						direction: Some(Direction::In),
		// 						typ: Type {
		// 							name: Identifier { name: "type".to_string().into(), length: 1 },
		// 							params: None
		// 						},
		// 						name: Identifier { name: "int_param".to_string().into(), length: 1 },
		// 						length: 7
		// 					}]
		// 				}
		// 			}),
		// 			length: 13
		// 		}]
		// 	}
		// );

		Ok(())
	}
}
