use analyzer_core::base_abstractions::FileId;
use analyzer_abstractions::lsp_types::Url;

pub(crate) type ParsedUnit = FileId;

pub(crate) trait BackgroundLoad {
	fn load(&self, file_path: Url);
}

pub(crate) type AnyBackgroundLoad = dyn BackgroundLoad + Send + Sync;
