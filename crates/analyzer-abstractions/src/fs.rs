use lsp_types::{TextDocumentIdentifier, Url};
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::BoxFuture;

/// Provides services that enumerate the contents of folders and files.
pub trait EnumerableFileSystem {
	// Why not use #[async_trait]?
	// The `async_trait` macro doesn't generate the Futures returned by the methods as `Sync`. Generally, this
	// is the correct behavior, but where these Futures are further used within `async` methods they will have
	// a requirement to be `Sync`. See the following issue:
	//
	// https://github.com/dtolnay/async-trait/issues/77
	//
	// In the future, we should look at possibly implementing our own as part of `analyzer-abstractions`. In the
	// meantime, this trait and its implementations will explicitly return boxed futures rather than be made
	// `async`.

	/// Enumerates the contents of a given folder and returns zero or more [`TextDocumentIdentifier`]'s identifying
	/// its contained files.
	///
	/// `file_pattern` is file glob pattern like `'*.p4'` that will be matched on paths relative to `folder_uri`.
	fn enumerate_folder<'a>(
		&'a self,
		folder_uri: Url,
		file_pattern: String,
	) -> BoxFuture<'a, Vec<TextDocumentIdentifier>>;

	/// Retrieves the contents of a given file.
	fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>>;

	// Signals if the applied filesystem uses Native OS Reads or requests to LSP client
	fn is_native(&self) -> bool;

	// Allows dyn object to be turned to dyn Any for a concret type to be produced
	fn as_any(&mut self) -> &mut dyn Any;
}

pub type AnyEnumerableFileSystem = Box<dyn EnumerableFileSystem + Send + Sync + 'static>;
