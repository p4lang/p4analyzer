use lsp_types::Url;
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

	/// Enumerates the contents of a given folder returning zero or more [`FileSystemEntry`] describing those
	/// entries.
	fn enumerate_folder<'a>(&'a self, folder_uri: Url) -> BoxFuture<'a, Vec<FileSystemEntry>>;

	/// Retrieves the contents of a given file.
	fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>>;
}

/// Defines a file system entry type (i.e., a folder or a file).
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryType {
	Folder,
	File
}

/// Describes an entry that is part of a folder on the file system.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemEntry {
	/// The URI to the current entry.
	pub uri: Url,

	/// The type of the current entry (i.e., a folder or a file).
	#[serde(rename = "type")]
	pub typ: EntryType
}

pub type AnyEnumerableFileSystem = Box<dyn EnumerableFileSystem + Send + Sync + 'static>;
