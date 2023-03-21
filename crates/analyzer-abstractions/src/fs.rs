use async_trait::async_trait;
use lsp_types::Url;

#[async_trait]
pub trait EnumerableFileSystem {
	async fn enumerate_folder(&self, folder_uri: Url) -> Vec<FileSystemEntry>;

	async fn file_contents(&self, file_uri: Url) -> usize;
}

#[derive(Clone)]
pub enum EntryType {
	Folder,
	File
}

#[derive(Clone)]
pub struct FileSystemEntry {
	pub uri: Url,
	pub typ: EntryType
}

pub type AnyEnumerableFileSystem = Box<dyn EnumerableFileSystem + Send + Sync>;
