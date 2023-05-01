//! Abstracting concrete syntax trees into ASTs.

use std::{collections::HashMap, rc::Rc};

use anyhow::Result;

use super::{ast::*, p4_grammar::*, Cst, ExistingMatch, Rule};
use crate::Token;



#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn basic() -> Result<()> { Ok(()) }
}
