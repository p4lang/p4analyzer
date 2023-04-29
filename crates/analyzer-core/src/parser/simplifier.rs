//! Abstracting concrete syntax trees into ASTs

use anyhow::Result;

use crate::Token;
use super::{Cst, ExistingMatch};
use super::ast::*;
use super::p4_grammar::*;

pub fn simplify(cst: ExistingMatch<Token>) -> P4Program {
	match &*cst.cst {
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
	fn basic() -> Result<()> {
		Ok(())
	}
}
