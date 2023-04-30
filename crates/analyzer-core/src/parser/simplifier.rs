//! Abstracting concrete syntax trees into ASTs.

use std::{collections::HashMap, rc::Rc};

use anyhow::Result;

use super::{ast::*, p4_grammar::*, Cst, ExistingMatch, Rule};
use crate::Token;

pub fn simplify(grammar: Grammar, cst: ExistingMatch<P4GrammarRules, Token>) -> P4Program {
	simplify_internal(P4GrammarRules::start, &grammar[&P4GrammarRules::start], cst)
}

fn simplify_internal(rule_name: P4GrammarRules, rule: &Rule<P4GrammarRules, Token>, cst: ExistingMatch<P4GrammarRules, Token>) -> P4Program {
	// start => p4program
	if let Cst::Sequence(seq) = cst.cst {
		P4Program {
			top_level_declarations: seq
				.iter()
				.flat_map(|cst| match &cst.cst {
					Cst::Sequence(seq) => seq.iter(),
					_ => todo!(),
				})
				.map(|cst| simplify_top_level_declaration(&*cst))
				.collect(),
		}
	} else {
		panic!("CST top-level must be a sequence")
	}
}

fn simplify_top_level_declaration(cst: &ExistingMatch<P4GrammarRules, Token>) -> TopLevelDeclaration {
	match cst.cst {
		Cst::Terminal(_) => todo!(),
		Cst::Choice(_, _) => todo!(),
		Cst::Sequence(_) => todo!(),
		Cst::Repetition(_) => todo!(),
		Cst::Not(_) => todo!(),
		Cst::Nothing => todo!(),
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic() -> Result<()> { Ok(()) }
}
