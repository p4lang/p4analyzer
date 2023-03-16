use analyzer_abstractions::{fs::{EnumerableFileSystem, FileSystemEntry}, lsp_types::Url, async_trait::async_trait};

use crate::lsp::request::RequestManager;

#[derive(Clone)]
pub struct LspEnumerableFileSystem {
	// request_manager: RequestManager
}

impl LspEnumerableFileSystem {
	pub fn new(request_manager: RequestManager) -> Self {
		// Self { request_manager }
		Self {}
	}
}

#[async_trait]
impl EnumerableFileSystem for LspEnumerableFileSystem {
	async fn enumerate_folder(&self, folder_uri: Url) -> Vec<FileSystemEntry> {
		vec![]
	}

	async fn file_contents(&self, file_uri: Url) -> usize {
		10
	}
}
