use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

use crate::lexer::Token;

use super::{p4_grammar::P4GrammarRules, Cst, ExistingMatch, Rule};

// TODO: we want to mirror rust-analyzer's way of doing things. Both with the
// dynamically typed CST (less ceremony than what we have right now) and with
// the simplification into AST.

// TODO: specifically, we want zippers (SyntaxNode) and transient ASTs built on
// them.
// TODO: for that, we need to know what non-terminal underpins a SyntaxNode
// TODO: and we want to skip trivia nodes in traversals
//       1) let grammars identify trivia nodes (whitespace and comments, but also
//          unimportant intermediary non-terminals (which means both killing
//          children and just stepping over them))
//       2) make that classification accessible in SyntaxNodes, such that AST
//          traversal can actually check them. This is related to the pending
//          grammar refactor (a single map just doesn't cut it, and the initial
//          non-terminal should arguably be a part of it, rather than a separater
//          from_rules param)

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GreenNode(pub Rc<ExistingMatch<P4GrammarRules, Token>>);

// Rationale: the derived Hash is safe, because PartialEq passes for fewer
// SyntaxNode pairs than its derived counterpart. The offset comparison is there
// only to short-circuit in common cases.
// That is, `a == b => hash(a) == hash(b)` still holds, because the condition is
// weakened.
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Debug, Clone, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxNode(pub Rc<SyntaxData>);

impl PartialEq for SyntaxNode {
	fn eq(&self, other: &Self) -> bool { self.0.offset == other.0.offset && Rc::ptr_eq(&self.0, &other.0) }
}

pub type Grammar = super::Grammar<P4GrammarRules, Token>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxData {
	grammar: Grammar,
	offset: usize,
	node: GreenNode,
	/// Identifies both our parent and the rule through which we got here.
	pub parent: Option<(SyntaxNode, P4GrammarRules)>,
}

impl GreenNode {
	fn children<'a>(&'a self) -> Box<dyn Iterator<Item = GreenNode> + 'a> {
		use std::iter::{empty, once};

		match &self.0.cst {
			Cst::Nothing | Cst::Not(_) | Cst::Terminal(_) => Box::new(empty()),
			Cst::Choice(_, child) => Box::new(once(GreenNode(child.clone()))),
			Cst::Repetition(children) | Cst::Sequence(children) => Box::new(children.iter().cloned().map(GreenNode)),
		}
	}
}

impl SyntaxNode {
	pub fn new_root(grammar: Grammar, node: GreenNode) -> SyntaxNode {
		SyntaxNode(Rc::new(SyntaxData { grammar, offset: 0, node, parent: None }))
	}

	pub fn parent(&self) -> Option<SyntaxNode> { self.0.parent.clone().map(|(p, _)| p) }

	pub fn children(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
		use std::iter::{empty, once, repeat};
		// for a rule like
		// start => a, b, c;
		// we should return children corresponding to each node as well as the
		// non-terminal on the RHS, i.e. a, b, c.

		let rule = self.0.parent.as_ref().map(|(_, r)| r).unwrap_or(&self.0.grammar.initial);
		// inspect the CST and the grammar
		let rule_names: Box<dyn Iterator<Item = P4GrammarRules>> = match &self.0.grammar.rules[rule] {
			Rule::Terminal(_) => Box::new(empty()),
			Rule::TerminalPredicate(_, _) => Box::new(empty()),
			Rule::Choice(_) => Box::new(
				if let Cst::Choice(chosen, _) = self.0.node.0.cst {
					Box::new(once(chosen))
				} else {
					unreachable!("choice rule without choice in CST")
				},
			),
			Rule::Sequence(seq) => Box::new(seq.iter().cloned()),
			Rule::Repetition(rule) => Box::new(repeat(*rule)),
			Rule::Not(_) => Box::new(empty()),
			Rule::Nothing => Box::new(empty()),
		};

		rule_names.zip(self.0.node.children()).map(|(rule, child)| {
			SyntaxNode(Rc::new(SyntaxData {
				grammar: self.0.grammar.clone(),
				offset: self.0.offset,
				node: child,
				parent: Some((self.clone(), rule)),
			}))
		})
	}

	pub fn length(&self) -> usize { self.0.node.0.match_length }

	pub fn kind(&self) -> P4GrammarRules {
		if let Some((_, rule)) = self.0.parent {
			rule
		} else {
			self.0.grammar.initial
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct P4Program {
	pub top_level_declarations: Vec<TopLevelDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TopLevelDeclaration {
	pub annotations: Vec<Annotation>,
	pub kind: TopLevelDeclarationKind,
	/// The length of this node in the source token stream, in tokens.
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Annotation {
	Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TopLevelDeclarationKind {
	Parser(ParserDeclaration),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserDeclaration {
	pub parameters: ParameterList,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParameterList {
	pub list: Vec<Parameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Parameter {
	pub annotations: Vec<Annotation>,
	pub direction: Option<Direction>,
	pub typ: Type,
	pub name: Identifier,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
	In,
	Out,
	InOut,
	Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identifier {
	pub name: Rc<String>,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Type {
	pub name: Identifier,
	pub params: Option<TypeParameters>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameters {
	pub list: Vec<TypeParameter>,
	pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParameter {
	pub annotations: Vec<Annotation>,
	pub typ: Type,
	pub length: usize,
}
