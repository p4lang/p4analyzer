pub use logos::Span;

/// The input buffer.
#[salsa::input]
pub struct Buffer {
	#[return_ref]
	pub contents: String,
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
