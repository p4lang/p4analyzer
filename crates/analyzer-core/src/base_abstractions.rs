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

/// An accumulated collection of dependencies typically keyed by their source [`FileId`].
#[salsa::accumulator]
pub struct IncludedDependencies(IncludedDependency);

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

/// Represents an included dependency.
#[derive(Clone, PartialEq, Eq)]
pub struct IncludedDependency {
	/// The [`FileId`] of the included target.
	pub file: FileId,

	/// The path of the included target.
	pub include_path: String,
}
