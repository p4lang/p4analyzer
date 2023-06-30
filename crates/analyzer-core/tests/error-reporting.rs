extern crate analyzer_core;
use std::collections::HashMap;

use analyzer_core::*;
use analyzer_core::base_abstractions::*;
use analyzer_core::parser::ast::{preorder, SyntaxNode};
use analyzer_core::parser::p4_grammar;

fn parse_str(s: &str) -> Option<parser::ast::GreenNode> {
	let db = Database::default();
	let buf = Buffer::new(&db, s.to_string());
	let file_id = FileId::new(&db, "foo.p4".to_string());
	let fs = Fs::new(&db, [(file_id, buf)].into());
	parse(&db, fs, file_id)
}

#[test]
fn lexical_scoping() {
	let cst = parse_str(r"
	parser names(int very_very_long_name) {
		bool asdf = very_very_long_name;
		int x = 3;
		parser very_very_long_name() {}
	}
	");

	if let Some(cst) = cst {
		use parser::ast::*;
		let root = SyntaxNode::new_root(p4_grammar::get_grammar().into(), cst);
		for (depth, node) in preorder(0, root.clone()) {
			// eprintln!("{:?}", node.kind())
			if let Some(ident) = Ident::cast(node) {
				eprintln!("{}", ident.as_str());
			}
		}

		for (env, node) in lexical_preorder_v1([].into(), root) {
			eprintln!("{:?}", (env.into_iter().map(|(k, v)| (k, v.kind())).collect::<HashMap<_, _>>(), node.kind()));
		}
	} else {
		unreachable!()
	}
}

#[test]
fn basic() {
	// TODO: simulate input in an editor by gradually typing a line of source
	// code, check that the reported errors match the incomplete line
}
