use std::{
	collections::{BTreeMap, HashMap},
	rc::Rc,
};

use paste::paste;

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
	pub grammar: Grammar,
	pub offset: usize,
	pub node: GreenNode,
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

pub trait AstNode {
	fn can_cast(node: &SyntaxNode) -> bool;

	fn cast(node: SyntaxNode) -> Option<Self>
	where
		Self: Sized;

	fn syntax(&self) -> &SyntaxNode;
}

macro_rules! ast_node {
	($non_terminal:ident) => {
		paste! {
			#[doc = "AST node for [`P4GrammarRules::" $non_terminal "`]."]
			#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
			pub struct [<$non_terminal:camel>] {
				syntax: SyntaxNode,
			}

			impl AstNode for [<$non_terminal:camel>] {
				fn can_cast(node: &SyntaxNode) -> bool { node.kind() == P4GrammarRules::$non_terminal }

				fn cast(node: SyntaxNode) -> Option<Self> {
					if node.kind() == P4GrammarRules::$non_terminal {
						Some(Self { syntax: node })
					} else {
						None
					}
				}

				fn syntax(&self) -> &SyntaxNode { &self.syntax }
			}
		}
	};
}

macro_rules! ast_methods {
	($non_terminal:ident, $($method:ident),+) => {
		paste! {
			impl [<$non_terminal:camel>] {
				$(
					#[doc = "Fetch the `" $method "` child of this node."]
					pub fn $method(&self) -> impl Iterator<Item = [<$method:camel>]> + '_ {
						self.syntax.children().flat_map(|child| {
							// FIXME: this only works on one level, needs to
							// proceed until it meets a castable successor, but
							// really this should follow the grammar definition
							// instead, otherwise we could run into false
							// positives further down the tree, not to mention
							// exhaustive search in the entire subtree that will
							// slow things down substantially
							let b: Box<dyn Iterator<Item = SyntaxNode>> = if ![<$method:camel>]::can_cast(&child) {
								Box::new(child.children().collect::<Vec<_>>().into_iter())
							} else {
								Box::new(std::iter::once(child))
							};
							b
						}).filter_map([<$method:camel>]::cast)
					}
				)+
			}
		}
	};
}

ast_node!(parser_decl);
ast_methods!(parser_decl, parameter_list);

// impl ParserDecl {
// 	pub fn parameter_list(&self) -> Option<ParameterList> {
// 		self.syntax.children().find_map(ParameterList::cast)
// 	}
// }

ast_node!(parameter_list);
ast_methods!(parameter_list, parameter);

ast_node!(parameter);
ast_methods!(parameter, direction, typ, ident);

ast_node!(direction);
ast_node!(typ);
ast_node!(ident);
