use anyhow::{anyhow, Result};
use parking_lot::{RwLock, RwLockReadGuard};
use std::{collections::HashMap, rc::Rc};

#[derive(Debug, Default)]
pub struct Parser<Token: Clone> {
	rules: Rc<HashMap<RuleName, Rule<Token>>>,
	buffer: RwLock<Vec<Token>>,
	memo_table: Vec<Column<Token>>,
}

#[derive(Debug)]
pub struct Matcher<'a, Token: Clone> {
	rules: Rc<HashMap<RuleName, Rule<Token>>>,
	memo_table: &'a mut Vec<Column<Token>>,
	input: RwLockReadGuard<'a, Vec<Token>>,
	pos: usize,
	max_examined_pos: isize,
}

type RuleName = &'static str;

#[derive(Debug, Clone)]
struct Column<Token: Clone> {
	memo: HashMap<RuleName, MemoTableEntry<Token>>,
	max_examined_length: isize,
}

impl<T: Clone> Default for Column<T> {
	fn default() -> Self { Self { memo: Default::default(), max_examined_length: -1 } }
}

#[derive(Debug, Clone)]
struct MemoTableEntry<Token: Clone> {
	existing_match: Option<ExistingMatch<Token>>,
	examined_length: usize,
}

#[derive(Debug, Clone)]
struct ExistingMatch<Token: Clone> {
	cst: Rc<Cst<Token>>,
	match_length: usize,
}

/// The concrete syntax tree type exactly mirrors the structure of the grammar.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum Cst<Token: Clone> {
	Terminal(Rc<Vec<Token>>),
	// TODO: switch to refcounting to share memoized stuff
	Choice(RuleName, Rc<Cst<Token>>),
	Sequence(Vec<Rc<Cst<Token>>>),
	Repetition(Vec<Rc<Cst<Token>>>),
	Not(RuleName),
}

impl<Token: Clone + PartialEq> Parser<Token> {
	pub fn from_rules<R: Into<HashMap<RuleName, Rule<Token>>> + Clone>(
		rules: &R,
	) -> Result<impl FnOnce(RwLock<Vec<Token>>) -> Parser<Token>> {
		let rules: HashMap<_, _> = rules.clone().into();
		if !rules.contains_key("start") {
			return Err(anyhow!("Missing initial non-terminal 'start'"));
		}

		let neighbours = |rule: &Rule<Token>| match rule {
			Rule::Terminal(_) => vec![],
			Rule::Choice(options) => options.clone(),
			Rule::Sequence(parts) => parts.clone(),
			Rule::Repetition(rule) => vec![*rule],
			Rule::Not(rule) => vec![*rule],
		};

		for (k, rule) in rules.iter() {
			if let Some(n) = neighbours(rule).iter().find(|name| !rules.contains_key(*name)) {
				return Err(anyhow!("Rule '{k}' references undefined '{n}'"));
			}
		}

		Ok(move |buffer| Parser { rules: rules.into(), memo_table: vec![], buffer })
	}

	pub fn _match(&mut self) -> Option<Cst<Token>> {
		let mut matcher = Matcher {
			rules: self.rules.clone(),
			memo_table: &mut self.memo_table,
			input: self.buffer.read(),
			pos: 0,
			max_examined_pos: -1,
		};

		matcher
			.memoized_eval_rule("start")
			.filter(|_| matcher.pos == matcher.input.len())
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

		fn invalidate_entries_in_column<Tk: Clone>(col: &mut Column<Tk>, pos: usize, start_pos: usize) {
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

impl<'a, Token: Clone + PartialEq> Matcher<'a, Token> {
	// originally under the (weird?) RuleApplication abstraction
	fn memoized_eval_rule(&mut self, rule_name: RuleName) -> Option<Rc<Cst<Token>>> {
		if let Some(cst) = self.use_memoized_result(rule_name) {
			Some(cst)
		} else {
			let orig_pos = self.pos;
			let orig_max = self.max_examined_pos;
			self.max_examined_pos = -1;

			let cst = self.eval_rule(rule_name);
			self.memoize_result(orig_pos, rule_name, cst.clone());

			self.max_examined_pos = self.max_examined_pos.max(orig_max);
			cst
		}
	}

	// originally a Rule method
	fn eval_rule(&mut self, rule_name: RuleName) -> Option<Rc<Cst<Token>>> {
		let rules = self.rules.clone();
		match &rules[rule_name] {
			Rule::Terminal(vec) => {
				for tk in vec.iter() {
					if !self.consume(tk) {
						return None;
					}
				}

				Some(Cst::Terminal(vec.clone()).into())
			}
			Rule::Choice(options) => {
				let orig_pos = self.pos;
				for rule in options {
					self.pos = orig_pos;
					if let Some(cst) = self.memoized_eval_rule(rule) {
						return Some(Cst::Choice(rule, cst).into());
					}
				}
				None
			}
			Rule::Sequence(parts) => {
				let mut matches = vec![];
				for rule in parts {
					if let Some(cst) = self.memoized_eval_rule(rule) {
						if matches.capacity() == 0 {
							matches.reserve_exact(parts.len())
						}

						matches.push(cst);
					} else {
						return None;
					}
				}

				Some(Cst::Sequence(matches).into())
			}
			Rule::Repetition(rule) => {
				let mut matches = vec![];
				loop {
					let orig_pos = self.pos;
					if let Some(cst) = self.memoized_eval_rule(rule) {
						matches.push(cst);
					} else {
						self.pos = orig_pos;
						break Some(Cst::Repetition(matches).into());
					}
				}
			}
			Rule::Not(rule) => {
				let orig_pos = self.pos;
				if self.memoized_eval_rule(rule).is_some() {
					None
				} else {
					self.pos = orig_pos;
					Some(Cst::Not(rule).into())
				}
			}
		}
	}

	fn memoize_result(&mut self, pos: usize, rule_name: RuleName, cst: Option<Rc<Cst<Token>>>) {
		while self.memo_table.len() <= pos {
			self.memo_table.push(Default::default());
		}

		let col = &mut self.memo_table[pos];
		let examined_length = (self.max_examined_pos - pos as isize + 1) as usize;
		let existing_match = cst.map(|cst| ExistingMatch { cst, match_length: self.pos - pos });

		let entry = MemoTableEntry { existing_match, examined_length };

		col.memo.insert(rule_name, entry);
		col.max_examined_length = col.max_examined_length.max(examined_length as isize)
	}

	fn use_memoized_result(&mut self, rule_name: RuleName) -> Option<Rc<Cst<Token>>> {
		self.memo_table.get(self.pos).and_then(|col| {
			col.memo.get(rule_name).and_then(|entry| {
				self.max_examined_pos = self.max_examined_pos.max((self.pos + entry.examined_length - 1) as isize);

				entry.existing_match.clone().map(|m| {
					self.pos += m.match_length;
					m.cst
				})
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
}

#[derive(Debug, Clone)]
pub enum Rule<Token: Clone> {
	Terminal(Rc<Vec<Token>>),
	Choice(Vec<RuleName>),
	Sequence(Vec<RuleName>),
	Repetition(RuleName),
	Not(RuleName),
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
	use pretty_assertions::{assert_eq, assert_ne};

	#[test]
	fn terminal() {
		let matcher =
			Parser::from_rules(&[("start", Rule::Terminal("foo".chars().collect::<Vec<_>>().into()))]).unwrap();

		let result = matcher("foo".chars().collect::<Vec<_>>().into())._match();
		assert_eq!(result, Some(Cst::Terminal("foo".chars().collect::<Vec<_>>().into())));
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
			._match()
		};

		let input = "1".chars().collect::<Vec<_>>().into();
		assert_eq!(mtch(input), Some(Cst::Choice("b", Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into())));

		let input = "2".chars().collect::<Vec<_>>().into();
		assert_eq!(
			mtch(input),
			Some(Cst::Choice(
				"a",
				Cst::Choice("x", Cst::Terminal("2".chars().collect::<Vec<_>>().into()).into()).into()
			))
		);
		assert_eq!(
			mtch("3".chars().collect::<Vec<_>>().into()),
			Some(Cst::Choice(
				"a",
				Cst::Choice("y", Cst::Terminal("3".chars().collect::<Vec<_>>().into()).into()).into()
			))
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
			matcher("1foo".chars().collect::<Vec<_>>().into())._match(),
			Some(Cst::Sequence(vec![
				Cst::Terminal("1".chars().collect::<Vec<_>>().into()).into(),
				Cst::Choice("y", Cst::Terminal("foo".chars().collect::<Vec<_>>().into()).into()).into()
			]))
		);
	}

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
			parser._match(),
			Some(Cst::Choice(
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
			parser._match(),
			Some(Cst::Choice(
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
			parser._match(),
			Some(Cst::Choice(
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
		assert_eq!(parser._match(), None);

		apply_edit(&mut parser, 3..3, "123");
		// "42+123"
		assert_eq!(
			parser._match(),
			Some(Cst::Choice(
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
			parser._match(),
			Some(Cst::Choice(
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
		assert_eq!(parser._match(), None,);

		apply_edit(&mut parser, 3..4, "0-0");
		// "9420-0123"
		assert_eq!(
			parser._match(),
			Some(Cst::Choice(
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
}
