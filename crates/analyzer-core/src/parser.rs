use anyhow::{anyhow, Result};
use parking_lot::{RwLock, RwLockReadGuard};
use std::{collections::HashMap, rc::Rc};

use crate::extensions::*;

mod p4_grammar;

#[derive(Debug, Default)]
pub struct Parser<Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> {
	rules: Rc<HashMap<RuleName, Rule<Token>>>,
	buffer: RwLock<Vec<Token>>,
	memo_table: Vec<Column<Token>>,
}

#[derive(Debug)]
pub struct Matcher<'a, Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> {
	rules: Rc<HashMap<RuleName, Rule<Token>>>,
	memo_table: &'a mut Vec<Column<Token>>,
	input: RwLockReadGuard<'a, Vec<Token>>,
	pos: usize,
	max_examined_pos: isize,
}

type RuleName = &'static str;

#[derive(Debug, Clone)]
struct Column<Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> {
	memo: HashMap<RuleName, MemoTableEntry<Token>>,
	max_examined_length: isize,
}

impl<T: std::fmt::Debug + PartialEq + PartialOrd + Clone> Default for Column<T> {
	fn default() -> Self { Self { memo: Default::default(), max_examined_length: -1 } }
}

#[derive(Debug, Clone)]
struct MemoTableEntry<Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> {
	existing_match: Result<Rc<ExistingMatch<Token>>, ParserError<Token>>,
	examined_length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExistingMatch<Token: Clone> {
	cst: Rc<Cst<Token>>,
	match_length: usize,
}

/// The concrete syntax tree type exactly mirrors the structure of the grammar.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Cst<Token: Clone> {
	Terminal(Rc<Vec<Token>>),
	Choice(RuleName, Rc<ExistingMatch<Token>>),
	Sequence(Vec<Rc<ExistingMatch<Token>>>),
	Repetition(Vec<Rc<ExistingMatch<Token>>>),
	Not(RuleName),
	Nothing,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum ParserError<Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> {
	Expected(RuleName, Box<ParserError<Token>>),
	Unexpected(RuleName),
	ExpectedOneOf(Vec<(RuleName, ParserError<Token>)>),
	ExpectedEof,
	ExpectedPatternMatch(&'static str),
	ExpectedToken(Token),
}

impl<Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> Parser<Token> {
	pub fn from_rules<R: Into<HashMap<RuleName, Rule<Token>>> + Clone>(
		rules: &R,
	) -> Result<impl FnOnce(RwLock<Vec<Token>>) -> Parser<Token>> {
		let rules: HashMap<_, _> = rules.clone().into();
		if !rules.contains_key("start") {
			return Err(anyhow!("Missing initial non-terminal 'start'"));
		}

		let neighbours = |rule: &Rule<Token>| match rule {
			Rule::Terminal(_) | Rule::TerminalPredicate(..) => vec![],
			Rule::Choice(options) => options.clone(),
			Rule::Sequence(parts) => parts.clone(),
			Rule::Repetition(rule) => vec![*rule],
			Rule::Not(rule) => vec![*rule],
			Rule::Nothing => vec![],
		};

		for (k, rule) in rules.iter() {
			if let Some(n) = neighbours(rule).iter().find(|name| !rules.contains_key(*name)) {
				return Err(anyhow!("Rule '{k}' references undefined '{n}'"));
			}
		}

		Ok(move |buffer| Parser { rules: rules.into(), memo_table: vec![], buffer })
	}

	pub fn parse(&mut self) -> Result<ExistingMatch<Token>, ParserError<Token>> {
		let mut matcher = Matcher {
			rules: self.rules.clone(),
			memo_table: &mut self.memo_table,
			input: self.buffer.read(),
			pos: 0,
			max_examined_pos: -1,
		};

		matcher
			.memoized_eval_rule("start")
			.filter(ParserError::ExpectedEof, |_| matcher.pos == matcher.input.len())
			.map(|rc| (*rc).clone())
	}

	/// Apply an edit operation, replacing the given `range` of tokens with `r`.
	pub fn apply_edit(&mut self, range: std::ops::Range<usize>, r: &[Token]) {
		// apply edit to the input
		self.buffer.write().splice(range.clone(), r.iter().cloned());

		// adjust the memo table: replace the affected range with empty entries
		self.memo_table.splice(range.clone(), std::iter::repeat(Default::default()).take(r.len()));

		// invalidate overlapping entries
		for pos in 0..range.start {
			if let Some(col) = self.memo_table.get_mut(pos) {
				if pos as isize + col.max_examined_length > range.start as isize {
					invalidate_entries_in_column(col, pos, range.start);
				}
			}
		}

		fn invalidate_entries_in_column<Tk: std::fmt::Debug + PartialEq + PartialOrd + Clone>(
			col: &mut Column<Tk>,
			pos: usize,
			start_pos: usize,
		) {
			let mut new_max = 0;
			let mut to_remove = vec![];
			for (rule_name, entry) in &col.memo {
				if pos + entry.examined_length > start_pos {
					// this entry's "input range" overlaps the edit
					to_remove.push(*rule_name);
				} else if entry.examined_length > new_max {
					new_max = entry.examined_length;
				}
			}

			for k in to_remove {
				// remove all the affected memoized entries
				col.memo.remove(&k);
			}

			col.max_examined_length = new_max as isize;
		}
	}
}

impl<'a, Token: std::fmt::Debug + PartialEq + PartialOrd + Clone> Matcher<'a, Token> {
	// originally under the (weird?) RuleApplication abstraction
	fn memoized_eval_rule(&mut self, rule_name: RuleName) -> Result<Rc<ExistingMatch<Token>>, ParserError<Token>> {
		if let Some(cst) = self.use_memoized_result(rule_name) {
			cst
		} else {
			let orig_pos = self.pos;
			let orig_max = self.max_examined_pos;
			self.max_examined_pos = -1;

			let cst = self.eval_rule(rule_name);
			let r = self.memoize_result(orig_pos, rule_name, cst.clone());

			self.max_examined_pos = self.max_examined_pos.max(orig_max);
			r
		}
	}

	// originally a Rule method
	fn eval_rule(&mut self, rule_name: RuleName) -> Result<Rc<Cst<Token>>, ParserError<Token>> {
		let rules = self.rules.clone();
		match &rules[rule_name] {
			Rule::Nothing => {
				self.max_examined_pos = self.max_examined_pos.max(self.pos as isize - 1);
				Ok(Cst::Nothing.into())
			}
			Rule::Terminal(vec) => {
				for tk in vec.iter() {
					if !self.consume(tk) {
						return Err(ParserError::ExpectedToken(tk.clone()));
					}
				}

				Ok(Cst::Terminal(vec.clone()).into())
			}
			Rule::TerminalPredicate(f, pattern) => self
				.skip()
				.cloned()
				.filter(f)
				.map(|tk| Rc::new(Cst::Terminal(vec![tk].into())))
				.ok_or(ParserError::ExpectedPatternMatch(pattern)),
			Rule::Choice(options) => {
				let orig_pos = self.pos;
				let mut errors = vec![];

				for rule in options {
					self.pos = orig_pos;
					match self.memoized_eval_rule(rule) {
						Ok(cst) => return Ok(Cst::Choice(rule, cst).into()),
						Err(e) => errors.push((*rule, e)),
					}
				}

				Err(ParserError::ExpectedOneOf(errors))
			}
			Rule::Sequence(parts) => {
				let mut matches = vec![];
				for rule in parts {
					let result = self.memoized_eval_rule(rule);
					match result {
						Ok(cst) => {
							if matches.capacity() == 0 {
								matches.reserve_exact(parts.len())
							}

							matches.push(cst);
						}
						Err(e) => return Err(ParserError::Expected(rule, e.into())),
					}
				}

				Ok(Cst::Sequence(matches).into())
			}
			Rule::Repetition(rule) => {
				let mut matches = vec![];
				loop {
					let orig_pos = self.pos;
					if let Ok(cst) = self.memoized_eval_rule(rule) {
						matches.push(cst);
					} else {
						self.pos = orig_pos;
						break Ok(Cst::Repetition(matches).into());
					}
				}
			}
			Rule::Not(rule) => {
				let orig_pos = self.pos;
				if self.memoized_eval_rule(rule).is_ok() {
					Err(ParserError::Unexpected(rule))
				} else {
					self.pos = orig_pos;
					Ok(Cst::Not(rule).into())
				}
			}
		}
	}

	fn memoize_result(
		&mut self,
		pos: usize,
		rule_name: RuleName,
		cst: Result<Rc<Cst<Token>>, ParserError<Token>>,
	) -> Result<Rc<ExistingMatch<Token>>, ParserError<Token>> {
		while self.memo_table.len() <= pos {
			self.memo_table.push(Default::default());
		}

		let col = &mut self.memo_table[pos];
		let examined_length = (self.max_examined_pos - pos as isize + 1) as usize;
		let existing_match = cst.map(|cst| Rc::new(ExistingMatch { cst, match_length: self.pos - pos }));

		let entry = MemoTableEntry { existing_match: existing_match.clone(), examined_length };

		col.memo.insert(rule_name, entry);
		col.max_examined_length = col.max_examined_length.max(examined_length as isize);

		existing_match
	}

	fn use_memoized_result(
		&mut self,
		rule_name: RuleName,
	) -> Option<Result<Rc<ExistingMatch<Token>>, ParserError<Token>>> {
		self.memo_table.get(self.pos).and_then(|col| {
			col.memo.get(rule_name).and_then(|entry| {
				self.max_examined_pos = self.max_examined_pos.max((self.pos + entry.examined_length - 1) as isize);

				Some(entry.existing_match.clone().map(|m| {
					self.pos += m.match_length;
					m
				}))
			})
		})
	}

	fn consume(&mut self, tk: &Token) -> bool {
		self.max_examined_pos = self.max_examined_pos.max(self.pos as isize);

		if self.input.get(self.pos) == Some(tk) {
			self.pos += 1;
			true
		} else {
			false
		}
	}

	fn skip(&mut self) -> Option<&Token> {
		self.max_examined_pos = self.max_examined_pos.max(self.pos as isize);

		if let Some(tk) = self.input.get(self.pos) {
			self.pos += 1;
			Some(tk)
		} else {
			None
		}
	}
}

#[derive(Debug, Clone)]
pub enum Rule<Token: Clone> {
	Terminal(Rc<Vec<Token>>),
	TerminalPredicate(for<'a> fn(&'a Token) -> bool, &'static str),
	Choice(Vec<RuleName>),
	Sequence(Vec<RuleName>),
	Repetition(RuleName),
	Not(RuleName),
	Nothing,
}

#[macro_export]
macro_rules! rule_rhs {
	($lit:literal) => {
		{
			let lit: &'static str = $lit;
			Rule::Terminal(lit.chars().collect::<Vec<_>>().into())
		}
	};
	($name:ident | $($names:ident)|+) => {
		Rule::Choice(vec![stringify!($name), $(stringify!($names)),+])
	};
	($name:ident, $($names:ident),+) => {
		Rule::Sequence(vec![stringify!($name), $(stringify!($names)),+])
	};
	($name:ident rep) => {
		Rule::Repetition(stringify!($name))
	};
	($expr:expr) => {
		Rule::Terminal($expr)
	};
}

#[macro_export]
macro_rules! grammar {
	($($name:ident =>
		$prefix:tt
		$(| $($or:tt)|+)?
		$(, $($seq:tt),+)?
		$($rep:ident)?
	);+$(;)?) => {
		[$((stringify!($name), rule_rhs!($prefix $(| $($or)|+)? $(, $($seq),+)? $($rep)?))),+]
	};
}

#[cfg(test)]
mod test {
	use super::*;
	use pretty_assertions::assert_eq;

	#[test]
	fn terminal() {
		let matcher =
			Parser::from_rules(&[("start", Rule::Terminal("foo".chars().collect::<Vec<_>>().into()))]).unwrap();

		let result = matcher("foo".chars().collect::<Vec<_>>().into()).parse();
		assert_eq!(
			result,
			Ok(ExistingMatch { cst: Cst::Terminal("foo".chars().collect::<Vec<_>>().into()).into(), match_length: 3 })
		);
	}

	#[test]
	fn choice_of_terminals() {
		let mtch = |input| {
			Parser::from_rules(&[
				("start", Rule::Choice(vec!["a", "b", "c"])),
				("a", Rule::Choice(vec!["x", "y"])),
				("b", Rule::Terminal("1".chars().collect::<Vec<_>>().into())),
				("c", Rule::Choice(vec!["b", "y"])),
				("x", Rule::Terminal("2".chars().collect::<Vec<_>>().into())),
				("y", Rule::Terminal("3".chars().collect::<Vec<_>>().into())),
			])
			.unwrap()(input)
			.parse()
		};

		let input = "1".chars().collect::<Vec<_>>().into();
		assert_eq!(
			mtch(input),
			Ok(ExistingMatch {
				cst: Cst::Choice(
					"b",
					ExistingMatch {
						cst: Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into(),
						match_length: 1,
					}
					.into()
				)
				.into(),
				match_length: 1,
			})
		);

		let input = "2".chars().collect::<Vec<_>>().into();
		assert_eq!(
			mtch(input),
			Ok(ExistingMatch {
				cst: Cst::Choice(
					"a",
					ExistingMatch {
						cst: Cst::Choice(
							"x",
							ExistingMatch {
								cst: Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into(),
								match_length: 1,
							}
							.into()
						)
						.into(),
						match_length: 1,
					}
					.into()
				)
				.into(),
				match_length: 1,
			})
		);

		assert_eq!(
			mtch("3".chars().collect::<Vec<_>>().into()),
			Ok(ExistingMatch {
				cst: Cst::Choice(
					"a",
					ExistingMatch {
						cst: Cst::Choice(
							"y",
							ExistingMatch {
								cst: Cst::Terminal("3".chars().collect::<Vec<_>>().into()).into(),
								match_length: 1
							}
							.into()
						)
						.into(),
						match_length: 1,
					}
					.into()
				)
				.into(),
				match_length: 1,
			})
		);
	}

	#[test]
	fn full_grammar() {
		let matcher = Parser::from_rules(&grammar! {
			start => a, b;
			b => a | y;
			a => "1";
			y => "foo";
		})
		.unwrap();

		assert_eq!(
			matcher("1foo".chars().collect::<Vec<_>>().into()).parse(),
			Ok(ExistingMatch {
				cst: Cst::Sequence(vec![
					ExistingMatch {
						cst: Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into(),
						match_length: 1,
					}
					.into(),
					ExistingMatch {
						cst: Cst::Choice(
							"y",
							ExistingMatch {
								cst: Cst::Terminal("foo".chars().collect::<Vec<_>>().into()).into(),
								match_length: 3,
							}
							.into()
						)
						.into(),
						match_length: 3,
					}
					.into(),
				])
				.into(),
				match_length: 4
			})
		);
	}

	/*

	#[test]
	fn simple_edit() {
		let buffer = "896-7".chars().collect::<Vec<_>>();
		let input = buffer.into();
		let mut parser = Parser::from_rules(&grammar! {
			start => addition | subtraction;
			addition => num, plus, num;
			subtraction => num, minus, num;
			plus => "+";
			minus => "-";
			num => digit, many_digits;
			many_digits => digit rep;
			digit => n0 | n1 | n2 | n3 | n4 | n5 | n6 | n7 | n8 | n9;
			n0 => "0";
			n1 => "1";
			n2 => "2";
			n3 => "3";
			n4 => "4";
			n5 => "5";
			n6 => "6";
			n7 => "7";
			n8 => "8";
			n9 => "9";
		})
		.unwrap()(input);

		let apply_edit = |p: &mut Parser<_>, r: std::ops::Range<usize>, s: &'static str| {
			let as_tokens: Vec<_> = s.chars().collect();
			p.apply_edit(r, &as_tokens);
		};

		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"subtraction",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n8", Cst::Terminal("8".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n9", Cst::Terminal("9".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n6", Cst::Terminal("6".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
					Cst::Terminal("-".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n7", Cst::Terminal("7".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![]).into()
					])
					.into(),
				])
				.into()
			))
		);

		apply_edit(&mut parser, 1..2, "0");

		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"subtraction",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n8", Cst::Terminal("8".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n0", Cst::Terminal("0".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n6", Cst::Terminal("6".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
					Cst::Terminal("-".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n7", Cst::Terminal("7".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![]).into()
					])
					.into(),
				])
				.into()
			))
		);

		apply_edit(&mut parser, 0..4, "42+");
		// the string is now "42+7"

		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"addition",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n4", Cst::Terminal("4".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![Cst::Choice(
							"n2",
							Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()
						)
						.into(),])
						.into()
					])
					.into(),
					Cst::Terminal("+".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n7", Cst::Terminal("7".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![]).into()
					])
					.into(),
				])
				.into()
			))
		);

		apply_edit(&mut parser, 3..4, "");
		// "42+"
		assert_eq!(parser.parse(), Err(ParserError::ExpectedEof));

		apply_edit(&mut parser, 3..3, "123");
		// "42+123"
		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"addition",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n4", Cst::Terminal("4".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![Cst::Choice(
							"n2",
							Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()
						)
						.into(),])
						.into()
					])
					.into(),
					Cst::Terminal("+".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n1", Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n2", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n3", Cst::Terminal("3".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
				])
				.into()
			))
		);

		apply_edit(&mut parser, 0..0, "9");
		// "942+123"
		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"addition",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n9", Cst::Terminal("9".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n4", Cst::Terminal("4".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n2", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
					Cst::Terminal("+".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n1", Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n2", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n3", Cst::Terminal("3".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
				])
				.into()
			))
		);

		apply_edit(&mut parser, 3..4, "_");
		// "942_123"
		assert_eq!(parser.parse(), Err(ParserError::ExpectedEof));

		apply_edit(&mut parser, 3..4, "0-0");
		// "9420-0123"
		assert_eq!(
			parser.parse(),
			Ok(Cst::Choice(
				"subtraction",
				Cst::Sequence(vec![
					Cst::Sequence(vec![
						Cst::Choice("n9", Cst::Terminal("9".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n4", Cst::Terminal("4".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n2", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n0", Cst::Terminal("0".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
					Cst::Terminal("-".chars().collect::<Vec<_>>().into()).into(),
					Cst::Sequence(vec![
						Cst::Choice("n0", Cst::Terminal("0".chars().collect::<Vec<_>>().into()).into()).into(),
						Cst::Repetition(vec![
							Cst::Choice("n1", Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n2", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into(),
							Cst::Choice("n3", Cst::Terminal("3".chars().collect::<Vec<_>>().into()).into()).into(),
						])
						.into()
					])
					.into(),
				])
				.into()
			))
		);
	}
	// */
}
