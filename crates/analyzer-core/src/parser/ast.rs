use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

use crate::lexer::Token;

use super::{p4_grammar::P4GrammarRules, Cst, ExistingMatch, Rule};

// TODO: we want to mirror rust-analyzer's way of doing things. Both with the
// dynamically typed CST (less ceremony than what we have right now) and with
// the simplification into AST.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GreenNode(pub Rc<ExistingMatch<P4GrammarRules, Token>>);

// Rationale: the derived Hash is safe, because PartialEq passes for fewer
// SyntaxNode pairs than its derived counterpart.
// That is, `a == b => hash(a) == hash(b)` still holds, because the condition is
// weakened.
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Debug, Clone, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxNode(pub Rc<SyntaxData>);

pub type Grammar = Rc<BTreeMap<P4GrammarRules, Rule<P4GrammarRules, Token>>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxData {
	grammar: Grammar,
	offset: usize,
	node: GreenNode,
	pub parent: Option<SyntaxNode>,
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

	pub fn parent(&self) -> Option<SyntaxNode> { self.0.parent.clone() }

	pub fn children(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
		self.0.node.children().map(|child| {
			SyntaxNode(Rc::new(SyntaxData {
				grammar: self.0.grammar.clone(),
				offset: self.0.offset,
				node: child,
				parent: Some(self.clone()),
			}))
		})
	}

	pub fn length(&self) -> usize { self.0.node.0.match_length }

	pub fn kind(&self) -> P4GrammarRules { todo!() }
}

impl PartialEq for SyntaxNode {
	fn eq(&self, other: &Self) -> bool { self.0.offset == other.0.offset && Rc::ptr_eq(&self.0, &other.0) }
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
