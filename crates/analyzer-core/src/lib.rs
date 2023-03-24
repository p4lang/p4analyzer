pub mod base_abstractions;
pub mod lexer;
pub mod parser;
pub mod preprocessor;

use std::collections::HashMap;

use logos::Logos;

use base_abstractions::*;
use lexer::*;
use preprocessor::*;

#[derive(Default)]
#[salsa::db(crate::Jar)]
pub struct Database {
	storage: salsa::Storage<Self>,
}

impl salsa::Database for Database {}

#[salsa::jar(db = Db)]
pub struct Jar(
	Buffer,
	LexedBuffer,
	FileId,
	Diagnostics,
	Fs,
	LexedFs,
	// gotta include salsa functions as well
	lex,
	preprocess,
);

pub trait Db: salsa::DbWithJar<Jar> {}

impl<DB> Db for DB where DB: ?Sized + salsa::DbWithJar<Jar> {}

#[derive(Default)]
pub struct Analyzer {
	db: Database,
	fs: Option<Fs>,
}

#[salsa::tracked]
pub struct LexedFs {
	fs: HashMap<FileId, LexedBuffer>,
}

#[salsa::input]
pub struct Fs {
	fs: HashMap<FileId, Buffer>,
}

impl Analyzer {
	fn filesystem(&self) -> HashMap<FileId, Buffer> {
		self.fs.map(|fs| fs.fs(&self.db)).unwrap_or_default()
	}

	pub fn update(&mut self, file_id: FileId, input: String) {
		let mut filesystem = self.filesystem();
		filesystem.insert(file_id, Buffer::new(&self.db, input));
		self.fs = Fs::new(&self.db, filesystem).into();
	}

	pub fn input(&self, file_id: FileId) -> Option<&str> {
		let buffer = self.buffer(file_id)?;
		Some(buffer.contents(&self.db))
	}

	pub fn buffer(&self, file_id: FileId) -> Option<Buffer> {
		self.filesystem().get(&file_id).copied()
	}

	pub fn lexed(&self, file_id: FileId) -> Option<&Vec<(Token, Span)>> {
		let lexed = lex(&self.db, file_id, *self.filesystem().get(&file_id)?);
		Some(lexed.lexemes(&self.db))
	}

	pub fn preprocessed(&self, file_id: FileId) -> Option<&Vec<(FileId, Token, Span)>> {
		preprocess(&self.db, self.fs?, file_id).as_ref()
	}

	pub fn diagnostics(&self, id: FileId) -> Vec<Diagnostic> {
		if let Some(buf) = self.filesystem().get(&id) {
			let mut d = lex::accumulated::<Diagnostics>(&self.db, id, *buf);
			d.append(&mut preprocess::accumulated::<Diagnostics>(&self.db, self.fs.unwrap(), id));
			d
		} else {
			vec![]
		}
	}

	pub fn delete(&mut self, uri: &str) -> Option<()> {
		let id = FileId::new(&self.db, uri.to_string());
		let mut filesystem = self.filesystem();
		filesystem.remove(&id).map(|_| ())?;
		self.fs = Fs::new(&self.db, filesystem).into();
		Some(())
	}

	pub fn file_id(&self, uri: &str) -> FileId {
		FileId::new(&self.db, uri.to_string())
	}

	pub fn path(&self, id: FileId) -> String {
		id.path(&self.db)
	}

	pub fn files(&self) -> Vec<String> {
		self.filesystem().keys().map(|k| k.path(&self.db)).collect()
	}
}

// TODO: trait for workspace logic?
//       - path resolution
//       - fetching unopened files
//       - change management in the fs
//       - see indexing in rust-analyzer
//       - instead of a "preprocessed filesystem,"
//         just rely on salsa's query caching

struct Lextender<'a>(Box<dyn Iterator<Item = (Token, Span)> + 'a>);

impl<'a> Iterator for Lextender<'a> {
	type Item = (Token, Span);

	fn next(&mut self) -> Option<Self::Item> {
		self.0.next()
	}
}

trait Lextensions<'a, 'db>: Iterator<Item = (Token, Span)> {
	fn process_error_tokens(self, db: &'db dyn crate::Db, file_id: FileId) -> Lextender<'a>
	where
		Self: Sized;
}

impl<'a, 'db> Lextensions<'a, 'db> for logos::SpannedIter<'a, Token>
where
	'db: 'a,
{
	fn process_error_tokens(self, db: &'db dyn crate::Db, file_id: FileId) -> Lextender<'a> {
		use itertools::{Itertools, Position};

		let scanner = Box::new(move |state: &mut Option<Span>, tk: (Token, Span)| {
			match (&*state, &tk) {
				(None, (Token::Error, span)) => *state = Some(span.clone()),
				(None, _) => (),
				(Some(err_span), (Token::Error, span)) => {
					*state = Some(err_span.start..span.end)
				}
				// the following arm will also hit if the original stream ends with Token::Error,
				// that's why we need to add one more token to the end of the stream (see below)
				(Some(err_span), _) => {
					// terminate the error range and emit a diagnostic
					let location = err_span.clone();
					*state = None;
					let diagnostic = Diagnostic {
						file: file_id,
						location,
						severity: Severity::Error,
						message: "unexpected token".to_string(),
					};

					Diagnostics::push(db, diagnostic);
				}
			};
			Some(tk)
		});

		let underlying = Box::new(
			self
				// add one more token at the end
				.chain(std::iter::once((Token::Whitespace, 0..0)))
				.scan(None, scanner)
				.with_position()
				// drop the last token
				.filter_map(|x| match x {
					Position::Last(_) | Position::Only(_) => None,
					_ => Some(x.into_inner()),
				}),
		);

		Lextender(underlying)
	}
}

#[salsa::tracked(return_ref)]
pub fn lex(db: &dyn crate::Db, file_id: FileId, buf: Buffer) -> LexedBuffer {
	let contents = buf.contents(db);
	let lexer = {
		let db = unsafe { std::mem::transmute(db) };
		Token::lexer_with_extras(contents, Lextras { db: Some(db), file_id })
	};

	// merge consecutive error tokens and push them as diagnostics
	// ("semantic" errors have already been pushed)
	let tokens: Vec<_> = lexer.spanned().process_error_tokens(db, file_id).collect();
	LexedBuffer::new(db, tokens)
}

#[salsa::tracked(return_ref)]
pub fn preprocess(db: &dyn crate::Db, fs: Fs, file_id: FileId) -> Option<Vec<(FileId, Token, Span)>> {
	let mut pp = PreprocessorState::new(
		|path: String| FileId::new(db, path),
		|file_id| {
			fs.fs(db).get(&file_id).map(|&buf| {
				let lexed = lex(db, file_id, buf);
				lexed.lexemes(db)
			})
		},
	);

	let fs = fs.fs(db);
	let buffer = fs.get(&file_id)?;
	let lexemes = lex(db, file_id, *buffer).lexemes(db);
	let mut input = lexemes.iter().cloned().map(|(tk, span)| (file_id, tk, span)).collect();
	let result = pp.preprocess(&mut input);

	dbg!(&pp.errors);

	for ((file, location), msg) in pp.errors {
		Diagnostics::push(
			db,
			Diagnostic {
				file,
				location,
				severity: Severity::Error,
				message: msg,
			},
		);
	}

	Some(result)
}
