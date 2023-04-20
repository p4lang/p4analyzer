use std::{cell::RefMut, sync::Arc};

use analyzer_core::base_abstractions::FileId;

use super::workspace::File;

pub(crate) type ParsedUnit = FileId;

pub(crate) trait Analyzer {
	fn unwrap(&self) -> RefMut<analyzer_core::Analyzer>;

	/// Enqueues a [`File`] for background analyzing.
	fn background_analyze(&self, file: Arc<File>);
}

pub(crate) type AnyAnalyzer = Box<dyn Analyzer + Send + Sync + 'static>;
