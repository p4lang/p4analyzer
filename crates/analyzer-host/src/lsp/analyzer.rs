use std::cell::RefMut;

use analyzer_abstractions::lsp_types::TextDocumentIdentifier;
use analyzer_core::base_abstractions::FileId;

pub(crate) type ParsedUnit = FileId;

pub(crate) trait Analyzer {
	fn unwrap(&self) -> RefMut<analyzer_core::Analyzer>;

	fn parse_text_document_contents(&self, document_identifier: TextDocumentIdentifier, contents: String)
		-> ParsedUnit;
}

pub(crate) type AnyAnalyzer = Box<dyn Analyzer + Send + Sync + 'static>;
