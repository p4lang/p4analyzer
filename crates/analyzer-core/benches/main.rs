mod lexer;
mod parser;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, lexer::criterion_benchmark, parser::criterion_benchmark);

criterion_main!(benches);
