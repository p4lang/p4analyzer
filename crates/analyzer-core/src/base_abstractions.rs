pub use logos::Span;

use crate::lsp_position::LspPos;

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
	/// The [`FileId`] identifying the dependency.
	pub file_id: FileId,

	/// A flag indicating the resolved state of the dependency.
	pub is_resolved: bool,
}
