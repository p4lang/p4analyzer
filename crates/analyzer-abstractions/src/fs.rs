use lsp_types::{TextDocumentIdentifier, Url};
use serde::{Deserialize, Serialize};

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
		file_pattern: String
	) -> BoxFuture<'a, Vec<TextDocumentIdentifier>>;

	/// Retrieves the contents of a given file.

	fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>>;
}

pub type AnyEnumerableFileSystem = Box<dyn EnumerableFileSystem + Send + Sync + 'static>;
