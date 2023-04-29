extern crate analyzer_core;

use analyzer_core::*;
use base_abstractions::*;
use parser::*;

use criterion::{black_box, Criterion};

fn baseline(make: impl FnOnce(String) -> Parser<char>, input: &str) -> Parser<char> {
	make(input.chars().into_iter().collect())
}

fn parse(make: impl FnOnce(String) -> Parser<char>, input: &str) -> ExistingMatch<char> {
	make(input.to_string()).parse().unwrap()
}

pub fn criterion_benchmark(c: &mut Criterion) {
	let mut group = c.benchmark_group("parser throughput");

	let rules = grammar! {
		start => num, expr_chain;
		expr_chain => expr_choice rep;
		expr_choice => add_chain | sub_chain;
		add_chain => plus, num;
		sub_chain => minus, num;
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
	};

	let input = {
		use oorandom::Rand32;
		let mut rand = Rand32::new(42);
		let mut buf = String::new();
		let mut last_op = true;

		// generate 100 kB of input
		for _ in 0..100_000 {
			let n = rand.rand_range(0..100) + 1;
			if !last_op && n < 20 {
				buf.push(if n % 2 == 0 { '+' } else { '-' });
				last_op = true;
			} else {
				let ch = (rand.rand_u32() % 10) as u8;
				buf.push((b'0' + ch) as char);
				last_op = false;
			}
		}

		buf
	};

	let mut matcher = Parser::from_rules(&rules).unwrap()(input.chars().collect::<Vec<_>>().into());
	assert!(matcher.parse().is_ok());

	group.bench_function("baseline", |b| {
		b.iter(|| {
			let make_matcher = Parser::from_rules(&rules).unwrap();
			let make = black_box(|s: String| make_matcher(s.chars().collect::<Vec<_>>().into()));
			baseline(make, black_box(&input))
		});
	});

	group.bench_function("parsing", |b| {
		b.iter(|| {
			let make_matcher = Parser::from_rules(&rules).unwrap();
			let make = black_box(|s: String| make_matcher(s.chars().collect::<Vec<_>>().into()));
			parse(make, black_box(&input))
		});
	});

	group.finish()
}
