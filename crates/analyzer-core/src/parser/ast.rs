use std::{
	collections::{BTreeMap, HashMap},
	ops::Range,
	rc::Rc,
};

use paste::paste;

use crate::lexer::Token;

use super::{p4_grammar::P4GrammarRules, Cst, ExistingMatch, Rule, TriviaClass};

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

pub type Grammar = Rc<super::Grammar<P4GrammarRules, Token>>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyntaxData {
	pub grammar: Grammar, // FIXME: Rc
	pub offset: usize,
	pub node: GreenNode,
	/// Identifies both our parent and the rule through which we got here.
	pub parent: Option<(SyntaxNode, P4GrammarRules)>,
}

impl GreenNode {
	fn children(&self) -> Box<dyn Iterator<Item = GreenNode>> {
		use std::iter::{empty, once};

		match &self.0.cst {
			Cst::Nothing | Cst::Not(_) | Cst::Terminal(_) => Box::new(empty()),
			Cst::Choice(_, child) => Box::new(once(GreenNode(child.clone()))),
			Cst::Repetition(children) | Cst::Sequence(children) => {
				Box::new(children.clone().into_iter().map(GreenNode))
			}
		}
	}
}

impl SyntaxNode {
	pub fn new_root(grammar: Grammar, node: GreenNode) -> SyntaxNode {
		SyntaxNode(Rc::new(SyntaxData { grammar, offset: 0, node, parent: None }))
	}

	pub fn parent(&self) -> Option<SyntaxNode> { self.0.parent.clone().map(|(p, _)| p) }

	pub fn children(&self) -> impl Iterator<Item = SyntaxNode> {
		use std::iter::{empty, once, repeat};

		let rule = self.0.parent.as_ref().map(|(_, r)| r).unwrap_or(&self.0.grammar.initial);
		// inspect the CST and the grammar
		let rule_names: Box<dyn Iterator<Item = P4GrammarRules>> = match &self.0.grammar.rules[rule] {
			Rule::Terminal(_) => Box::new(empty()),
			Rule::TerminalPredicate(_, _) => Box::new(empty()),
			Rule::Choice(_) => Box::new(if let Cst::Choice(chosen, _) = self.0.node.0.cst {
				Box::new(once(chosen))
			} else {
				unreachable!("choice rule without choice in CST")
			}),
			Rule::Sequence(seq) => Box::new(seq.clone().into_iter()), // FIXME: cloning
			Rule::Repetition(rule) => Box::new(repeat(*rule)),
			Rule::Not(_) => Box::new(empty()),
			Rule::Nothing => Box::new(empty()),
		};

		let parent = self.clone();
		let mut offset = self.0.offset;

		rule_names.zip(self.0.node.children()).map(move |(rule, child)| {
			let match_length = child.0.match_length;
			let r = SyntaxNode(Rc::new(SyntaxData {
				grammar: parent.0.grammar.clone(),
				offset,
				node: child,
				parent: Some((parent.clone(), rule)),
			}));
			offset += match_length;
			r
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

	pub fn trivia_class(&self) -> TriviaClass {
		self.0.grammar.trivia.get(&self.kind()).copied().unwrap_or(TriviaClass::Keep)
	}

	pub fn is_trivia(&self) -> bool { self.trivia_class() != TriviaClass::Keep }
}

pub fn preorder(depth: u32, node: SyntaxNode) -> Box<dyn Iterator<Item = (u32, SyntaxNode)>> {
	match node.trivia_class() {
		TriviaClass::Keep => Box::new(
			std::iter::once((depth, node.clone()))
				.chain(node.children().collect::<Vec<_>>().into_iter().flat_map(move |node| preorder(depth + 1, node))),
		),
		TriviaClass::SkipNodeOnly => {
			Box::new(node.children().collect::<Vec<_>>().into_iter().flat_map(move |node| preorder(depth, node)))
		}
		TriviaClass::SkipNodeAndChildren => Box::new(std::iter::empty()),
	}
}

pub trait AstNode {
	fn can_cast(node: &SyntaxNode) -> bool;

	fn cast(node: SyntaxNode) -> Option<Self>
	where
		Self: Sized;

	/// The backing SyntaxNode.
	fn syntax(&self) -> &SyntaxNode;

	/// The offset in tokens at which the node starts.
	fn offset(&self) -> usize { self.syntax().0.offset }

	/// The node's span in the token stream.
	fn span(&self) -> Range<usize> {
		let start = self.offset();
		let end = start + self.syntax().length();
		start..end
	}

	/// The node's span in the source code (in bytes).
	///
	/// This is a convenience method that calls [`AstNode::span()`] with the
	/// cumulative sum of token lengths.
	fn text_span(&self, cumulative_sum: &[usize]) -> Range<usize> {
		let Range { start, end } = self.span();
		let end = (end as isize - 1).max(0) as usize;
		if start == 0 {
			0..cumulative_sum[end]
		} else {
			cumulative_sum[start - 1]..cumulative_sum[end]
		}
	}
}

macro_rules! ast_node {
	($non_terminal:ident $(, methods: $($method:ident),+)?) => {
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

			$(
				impl [<$non_terminal:camel>] {
					$(
						#[doc = "Fetch the `" $method "` child of this node."]
						pub fn $method(&self) -> impl Iterator<Item = [<$method:camel>]> {
							// TODO: avoid allocation and dynamic dispatch (see
							// enum crates for possible help)
							fn go(node: SyntaxNode) -> Box<dyn Iterator<Item = SyntaxNode>> {
								match node.trivia_class() {
									TriviaClass::SkipNodeAndChildren => Box::new(std::iter::empty()),
									TriviaClass::SkipNodeOnly => Box::new(node.children().flat_map(go)),
									TriviaClass::Keep => Box::new(std::iter::once(node)),
								}
							}

							self.syntax().children().flat_map(go).filter_map([<$method:camel>]::cast)
						}
					)+
				}
			)?
		}
	};
}

ast_node!(parser_decl, methods: parameter_list);
ast_node!(parameter_list, methods: parameter);
ast_node!(parameter, methods: direction, typ, definition);
ast_node!(definition, methods: ident);

ast_node!(typ);
ast_node!(ident);

impl Ident {
	pub fn as_str(&self) -> &str {
		if let Cst::Terminal(tokens) = &self.syntax.0.node.0.cst {
			assert_eq!(tokens.len(), 1);
			if let Token::Identifier(s) = &tokens[0] {
				s
			} else {
				unreachable!("Ident AST node has a non-identifier token {:?}", tokens[0])
			}
		} else {
			unreachable!("Ident AST node has a non-terminal CST {:?}", self.syntax.0.node.0.cst)
		}
	}
}

/// AST node for [`P4GrammarRules::direction`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Direction {
	syntax: SyntaxNode,
	pub variant: DirectionTag,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DirectionTag {
	In,
	Out,
	InOut,
}

impl std::fmt::Display for DirectionTag {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			DirectionTag::In => write!(f, "in"),
			DirectionTag::Out => write!(f, "out"),
			DirectionTag::InOut => write!(f, "inout"),
		}
	}
}

impl AstNode for Direction {
	fn can_cast(node: &SyntaxNode) -> bool { node.kind() == P4GrammarRules::direction }

	fn cast(node: SyntaxNode) -> Option<Self> {
		if node.kind() == P4GrammarRules::direction {
			if let Cst::Choice(variant, _) = node.0.node.0.cst {
				Some(Self {
					syntax: node,
					variant: match variant {
						P4GrammarRules::dir_in => DirectionTag::In,
						P4GrammarRules::dir_out => DirectionTag::Out,
						P4GrammarRules::dir_inout => DirectionTag::InOut,
						_ => unreachable!("Direction CST chose a non-dir non-terminal"),
					},
				})
			} else {
				unreachable!("Direction AST node has a non-choice CST {:?}", node.0.node.0.cst)
			}
		} else {
			None
		}
	}

	fn syntax(&self) -> &SyntaxNode { &self.syntax }
}
