#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum PreprocessorQuotationStyle {
	AngleBrackets,
	DoubleQuotes,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorBinOp {
	Or,
	And,
	Plus,
	Minus,
	Times,
	Divide,
	Equals,
	NotEquals,
	LessThan,
	LessOrEqual,
	GreaterThan,
	GreaterOrEqual,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorExpression {
	IntLiteral(i64),
	Identifier(String),
	BinOp(
		PreprocessorBinOp,
		Box<PreprocessorExpression>,
		Box<PreprocessorExpression>,
	),
	Not(Box<PreprocessorExpression>),
	Defined(Box<PreprocessorExpression>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub enum PreprocessorDirective {
	Include(PreprocessorQuotationStyle, String),
	If(PreprocessorExpression),
	ElseIf(PreprocessorExpression),
	Else,
	EndIf,
	Define(String, String),
	Undef(String),
	Pragma(String),
	Other(String, String),
}

mod parser {
	use super::*;
	use nom::bytes::complete::tag;
	use nom::IResult;
	use nom::{
		branch::alt,
		character::complete::{alpha1, alphanumeric1, char, one_of},
		character::streaming::multispace0,
		combinator::{map, map_res, recognize},
		multi::{many0, many0_count, many1},
		sequence::{pair, preceded, separated_pair, terminated, tuple},
	};

	pub(super) fn identifier(input: &str) -> IResult<&str, &str> {
		recognize(pair(
			alt((alpha1, tag("_"))),
			many0_count(alt((alphanumeric1, tag("_")))),
		))(input)
	}

	fn decimal(input: &str) -> IResult<&str, i64> {
		map_res(
			recognize(many1(terminated(one_of("0123456789"), many0(char('_'))))),
			|out: &str| str::replace(out, "_", "").parse(),
		)(input)
	}

	fn hexadecimal(input: &str) -> IResult<&str, i64> {
		map_res(
			preceded(
				alt((tag("0x"), tag("0X"))),
				recognize(many1(terminated(
					one_of("0123456789abcdefABCDEF"),
					many0(char('_')),
				))),
			),
			|out: &str| i64::from_str_radix(&str::replace(out, "_", ""), 16),
		)(input)
	}

	fn integer(input: &str) -> IResult<&str, i64> {
		alt((hexadecimal, decimal))(input)
	}

	fn defined(input: &str) -> IResult<&str, PreprocessorExpression> {
		preceded(
			tuple((tag("defined"), multispace0, tag("("))),
			terminated(term, tag(")")),
		)(input)
	}

	pub(super) fn factor(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorExpression::*;
		alt((
			map(defined, |inner| Defined(inner.into())),
			map(identifier, |ident| Identifier(ident.to_string())),
			map(integer, IntLiteral),
		))(input)
	}

	pub(super) fn bin_op(
		subexpr: fn(&str) -> IResult<&str, PreprocessorExpression>,
		sym: &'static str,
		op: PreprocessorBinOp,
	) -> impl FnMut(&str) -> IResult<&str, PreprocessorExpression> {
		move |input| {
			map(
				separated_pair(
					subexpr,
					preceded(multispace0, terminated(tag(sym), multispace0)),
					subexpr,
				),
				|(l, r)| PreprocessorExpression::BinOp(op.clone(), l.into(), r.into()),
			)(input)
		}
	}

	pub(super) fn term(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		alt((
			bin_op(factor, "*", Times),
			bin_op(factor, "/", Divide),
			// TODO: others
			factor,
		))(input)
	}

	pub fn expression(input: &str) -> IResult<&str, PreprocessorExpression> {
		use PreprocessorBinOp::*;
		alt((
			bin_op(term, "+", Plus),
			bin_op(term, "-", Minus),
			// TODO: others,
			term,
		))(input)
	}
}

// TODO: is taking ownership necessary?
pub fn parse_pp_expression(buf: String) -> Option<PreprocessorExpression> {
	match parser::expression(&buf) {
		Ok((_, expr)) => Some(expr),
		Err(e) => {
			dbg!(e);
			None
		}
	}
}

#[cfg(test)]
mod test {
	use super::parser::*;
	use super::PreprocessorBinOp as Op;
	use super::PreprocessorExpression::*;
	use nom::IResult;
	use pretty_assertions::assert_eq;

	#[test]
	fn parse_factors() {
		assert_eq!(factor("foo"), Ok(("", Identifier("foo".to_string()))));
		assert_eq!(factor("123 asdf"), Ok((" asdf", IntLiteral(123))));
		assert_eq!(factor("0xff asdf"), Ok((" asdf", IntLiteral(255))));
	}

	#[test]
	fn parse_terms() {
		assert_eq!(term("foo"), Ok(("", Identifier("foo".to_string()))));
		assert_eq!(term("123 asdf"), Ok((" asdf", IntLiteral(123))));
		assert_eq!(term("0xff asdf"), Ok((" asdf", IntLiteral(255))));

		assert_eq!(
			term("foo * bar"),
			Ok((
				"",
				BinOp(
					Op::Times,
					Identifier("foo".to_string()).into(),
					Identifier("bar".to_string()).into()
				)
			))
		);
		assert_eq!(
			term("12 / asdf"),
			Ok((
				"",
				BinOp(
					Op::Divide,
					IntLiteral(12).into(),
					Identifier("asdf".to_string()).into()
				)
			))
		);
	}

	#[test]
	fn parse_expressions() {
		assert_eq!(
			expression("foo * bar x"),
			Ok((
				" x",
				BinOp(
					Op::Times,
					Identifier("foo".to_string()).into(),
					Identifier("bar".to_string()).into()
				)
			))
		);
		assert_eq!(
			expression("12 / asdf x"),
			Ok((
				" x",
				BinOp(
					Op::Divide,
					IntLiteral(12).into(),
					Identifier("asdf".to_string()).into()
				)
			))
		);
	}
}
