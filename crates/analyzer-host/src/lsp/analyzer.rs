use analyzer_abstractions::lsp_types::Url;
use analyzer_core::base_abstractions::FileId;

pub(crate) type ParsedUnit = FileId;

pub(crate) trait BackgroundLoad {
	fn load(&self, file_path: Url);
}

pub(crate) type AnyBackgroundLoad = dyn BackgroundLoad + Send + Sync;
