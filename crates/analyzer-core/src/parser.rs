use anyhow::{anyhow, Result};
use std::collections::HashMap;

#[derive(Default, Clone)]
pub struct Matcher {
	rules: std::rc::Rc<HashMap<RuleName, Rule>>,
	input: String,
	pos: usize,
	memo_table: HashMap<usize, Column>,
}

type RuleName = &'static str;
type Column = HashMap<RuleName, MemoTableEntry>;

#[derive(Clone)]
struct MemoTableEntry {
	cst: Option<Cst>,
	next_pos: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Cst {
	Terminal(String),
	Choice(RuleName, Box<Cst>),
	Sequence(Vec<Cst>),
	Repetition(Vec<Cst>),
	Not(RuleName),
}

impl Matcher {
	pub fn from_rules<R: Into<HashMap<RuleName, Rule>>>(rules: R) -> Result<Matcher> {
		let rules: HashMap<_, _> = rules.into();
		if !rules.contains_key("start") {
			return Err(anyhow!("Missing initial non-terminal 'start'"));
		}

		let neighbours = |rule: &Rule| match rule {
			Rule::Terminal(_) => vec![],
			Rule::Choice(options) => options.clone(),
			Rule::Sequence(parts) => parts.clone(),
			Rule::Repetition(rule) => vec![*rule],
			Rule::Not(rule) => vec![*rule],
		};

		for (k, rule) in rules.iter() {
			if let Some(n) = neighbours(rule)
				.iter()
				.find(|name| !rules.contains_key(*name))
			{
				return Err(anyhow!("Rule '{}' references undefined '{}'", k, n));
			}
		}

		Ok(Matcher {
			rules: rules.into(),
			..Default::default()
		})
	}

	pub fn _match(mut self, input: &str) -> Option<Cst> {
		self.pos = 0;
		self.input = input.to_string();

		self.memoized_eval_rule("start")
	}

	// originally under the (weird?) RuleApplication abstraction
	fn memoized_eval_rule(&mut self, rule_name: RuleName) -> Option<Cst> {
		if let Some(cst) = self.use_memoized_result(rule_name) {
			Some(cst)
		} else {
			let orig_pos = self.pos;
			let cst = self.eval_rule(rule_name);
			self.memoize_result(orig_pos, rule_name, cst.clone());
			cst
		}
	}

	// originally a Rule method
	fn eval_rule(&mut self, rule_name: RuleName) -> Option<Cst> {
		let rules = self.rules.clone();
		match &rules[rule_name] {
			Rule::Terminal(str) => {
				for char in str.chars() {
					if !self.consume(char) {
						return None;
					}
				}

				Some(Cst::Terminal(str.clone()))
			}
			Rule::Choice(options) => {
				let orig_pos = self.pos;
				for rule in options {
					eprintln!("  choice trying to match {rule}");
					self.pos = orig_pos;
					if let Some(cst) = self.eval_rule(rule) {
						return Some(Cst::Choice(rule, cst.into()));
					}
				}
				None
			}
			Rule::Sequence(parts) => {
				let mut matches = vec![];
				for rule in parts {
					if let Some(cst) = self.eval_rule(rule) {
						matches.push(cst);
					} else {
						return None;
					}
				}

				Some(Cst::Sequence(matches))
			}
			Rule::Repetition(rule) => {
				let mut matches = vec![];
				loop {
					let orig_pos = self.pos;
					if let Some(cst) = self.eval_rule(rule) {
						matches.push(cst);
					} else {
						self.pos = orig_pos;
						break Some(Cst::Repetition(matches));
					}
				}
			}
			Rule::Not(rule) => {
				let orig_pos = self.pos;
				if self.eval_rule(rule).is_some() {
					None
				} else {
					self.pos = orig_pos;
					Some(Cst::Not(rule))
				}
			}
		}
	}

	fn memoize_result(&mut self, pos: usize, rule_name: RuleName, cst: Option<Cst>) {
		let col = self.memo_table.entry(pos).or_default();
		col.insert(
			rule_name,
			MemoTableEntry {
				cst,
				next_pos: self.pos,
			},
		);
	}

	fn use_memoized_result(&self, rule_name: RuleName) -> Option<Cst> {
		self.memo_table
			.get(&self.pos)
			.and_then(|col| col.get(rule_name).and_then(|entry| entry.cst.clone()))
	}

	fn consume(&mut self, c: char) -> bool {
		if self.input.chars().nth(self.pos) == Some(c) {
			self.pos += 1;
			eprintln!("consumed {c}");
			true
		} else {
			eprintln!(
				"tried to consume {:?} but failed",
				self.input.chars().nth(self.pos)
			);
			false
		}
	}
}

#[derive(Debug, Clone)]
pub enum Rule {
	Terminal(String),
	Choice(Vec<RuleName>),
	Sequence(Vec<RuleName>),
	Repetition(RuleName),
	Not(RuleName),
}

#[cfg(test)]
mod test {
	use super::*;

	macro_rules! rule_rhs {
        ($lit:literal) => {
            {
                let lit: &'static str = $lit;
                Rule::Terminal(lit.to_string())
            }
        };
        ($name:ident | $($names:ident)|+) => {
            Rule::Choice(vec![stringify!($name), $(stringify!($names)),+])
        };
        ($name:ident, $($names:ident),+) => {
            Rule::Sequence(vec![stringify!($name), $(stringify!($names)),+])
        };
        ($name:ident*) => {
            Rule::Repetition(stringify!($name))
        };
        ($expr:expr) => {
            Rule::Terminal($expr)
        };
    }

	macro_rules! grammar {
        ($($name:ident =>
            $prefix:tt
            $(| $($or:tt)|+)?
            $(, $($seq:tt),+)?
        );+$(;)?) => {
            [$((stringify!($name), rule_rhs!($prefix $(| $($or)|+)? $(, $($seq),+)?))),+]
        };
    }

	#[test]
	fn terminal() {
		let matcher = Matcher::from_rules([("start", Rule::Terminal("foo".to_string()))]).unwrap();

		let result = matcher._match("foo");
		assert_eq!(result, Some(Cst::Terminal("foo".to_string())));
	}

	#[test]
	fn choice_of_terminals() {
		let matcher = Matcher::from_rules([
			("start", Rule::Choice(vec!["a", "b", "c"])),
			("a", Rule::Choice(vec!["x", "y"])),
			("b", Rule::Terminal("1".to_string())),
			("c", Rule::Choice(vec!["b", "y"])),
			("x", Rule::Terminal("2".to_string())),
			("y", Rule::Terminal("3".to_string())),
		])
		.unwrap();

		assert_eq!(
			matcher.clone()._match("1"),
			Some(Cst::Choice("b", Cst::Terminal("1".to_string()).into()))
		);
		assert_eq!(
			matcher.clone()._match("2"),
			Some(Cst::Choice(
				"a",
				Cst::Choice("x", Cst::Terminal("2".to_string()).into()).into()
			))
		);
		assert_eq!(
			matcher._match("3"),
			Some(Cst::Choice(
				"a",
				Cst::Choice("y", Cst::Terminal("3".to_string()).into()).into()
			))
		);
	}

	#[test]
	fn full_grammar() {
		let matcher = Matcher::from_rules(grammar! {
			start => a, b;
			b => a | y;
			a => "1";
			y => "foo";
		})
		.unwrap();

		assert_eq!(
			matcher._match("1foo"),
			Some(Cst::Sequence(vec![
				Cst::Terminal("1".to_string()),
				Cst::Choice("y", Cst::Terminal("foo".to_string()).into())
			]))
		);
	}
}
