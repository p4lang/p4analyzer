pub use logos::Span;

use crate::lsp_position_struct::LspPos;

/// The input buffer.
#[salsa::input]
pub struct Buffer {
	#[return_ref]
	pub contents: String,
	#[return_ref]
	pub byte_position: LspPos,
}

#[salsa::interned]
pub struct FileId {
	pub path: String,
}

#[salsa::accumulator]
pub struct Diagnostics(Diagnostic);

#[derive(Clone, PartialEq, Eq)]
pub enum Severity {
	Info,
	Hint,
	Warning,
	Error,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Diagnostic {
	pub file: FileId,
	pub location: std::ops::Range<usize>,
	pub severity: Severity,
	pub message: String,
}
