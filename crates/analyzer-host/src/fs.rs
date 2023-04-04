use analyzer_abstractions::{fs::EnumerableFileSystem, lsp_types::{Url, request::Request, TextDocumentIdentifier}, async_trait::async_trait, tracing::error, BoxFuture};
use serde::{Deserialize, Serialize};

use crate::lsp::request::RequestManager;

/// Provides services that enumerate the contents of folder and files.
///
/// [`LspEnumerableFileSystem`] delegates these services to the hosting LSP client by sending
/// extended requests using an extended method beginning `'p4analyzer/<...>'`.
#[derive(Clone)]
pub(crate) struct LspEnumerableFileSystem {
	request_manager: RequestManager
}

impl LspEnumerableFileSystem {
	/// Initializes a new [`LspEnumerableFileSystem`] with the request manager to use when sending
	/// requests.
	pub fn new(request_manager: RequestManager) -> Self {
		Self { request_manager }
	}
}

// #[async_trait]
impl EnumerableFileSystem for LspEnumerableFileSystem {
	fn enumerate_folder<'a>(&'a self, file_uri: Url, file_pattern: String) -> BoxFuture<'a, Vec<TextDocumentIdentifier>> {
		async fn enumerate_folder(s: &LspEnumerableFileSystem, folder_uri: Url, file_pattern: String) -> Vec<TextDocumentIdentifier> {
			let params = EnumerateFolderParams { uri: folder_uri.clone(), file_pattern };

			match s.request_manager.send_and_receive::<EnumerateFolderRequest>(params).await {
				Ok(entries) => entries,
				Err(err) => {
					error!(folder_uri = folder_uri.as_str(), "Failed to enumerate folder. {}", err);

					vec![]
				}
			}
		}

		Box::pin(enumerate_folder(self, file_uri, file_pattern))
	}

	fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>> {
		async fn file_contents(s: &LspEnumerableFileSystem, file_uri: Url) -> Option<String> {
			let params = TextDocumentIdentifier { uri: file_uri.clone() };

			match s.request_manager.send_and_receive::<FileContentsRequest>(params).await {
				Ok(contents) => Some(contents),
				Err(err) => {
					error!(file_uri = file_uri.as_str(), "Failed to retrieve file contents. {}", err);

					None
				}
			}
		}

		Box::pin(file_contents(self, file_uri))
	}
}

#[derive(Debug)]
pub(crate) enum EnumerateFolderRequest {}

impl Request for EnumerateFolderRequest {
	type Params = EnumerateFolderParams;
	type Result = Vec<TextDocumentIdentifier>;
	const METHOD: &'static str = "p4analyzer/enumerateFolder";
}

#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnumerateFolderParams {
	pub uri: Url,
	pub file_pattern: String

}

#[derive(Debug)]
pub(crate) enum FileContentsRequest {}

impl Request for FileContentsRequest {
	type Params = TextDocumentIdentifier;
	type Result = String;
	const METHOD: &'static str = "p4analyzer/fileContents";
}
