use core::fmt::Debug;
use std::{
	sync::{Arc, RwLock, Mutex},
	collections::{HashMap, hash_map::{Iter, IntoIter, Entry}}, fmt::{Formatter, Display, Result as FmtResult},
	task::Poll
};

use analyzer_abstractions::{
	lsp_types::{WorkspaceFolder, Url, TextDocumentIdentifier},
	fs::AnyEnumerableFileSystem,
	tracing::{error, info}, futures_extensions::async_extensions::AsyncPool
};
use analyzer_abstractions::futures_extensions::FutureCompletionSource;
use async_channel::{Sender, Receiver};
use thiserror::Error;

use super::progress::ProgressManager;

async fn index_files(index_receiver: Receiver<TextDocumentIdentifier>, file_system: Arc<AnyEnumerableFileSystem>) {
	info!("WorkspaceManager background indexing started.");

	loop {
		match index_receiver.recv().await {
			Ok(document_identifier) => {

				info!(file_uri = document_identifier.uri.as_str(), "Indexing");
			},
			Err(_) => break
		}
	}

	info!("WorkspaceManager background indexing stopped.");
}

/// Manages a collection of workspaces opened by an LSP compliant host.
#[derive(Clone)]
pub(crate) struct WorkspaceManager<T: Clone = ()> {
	has_workspaces: bool,
	workspaces: HashMap<Url, Arc<Workspace<T>>>,
}

impl<T> WorkspaceManager<T>
where
	T: Clone + Debug
{
	/// Initializes a new [`WorkspaceManager`] instance.
	///
	/// If `workspace_folders` is [`None`], then a root workspace folder will be used by default.
	pub fn new(file_system: Arc<AnyEnumerableFileSystem>, workspace_folders: Option<Vec<WorkspaceFolder>>) -> Self {
		fn to_workspace<T: Clone + Debug>(
			file_system: Arc<AnyEnumerableFileSystem>,
			workspace_folder: WorkspaceFolder,
			index_sender: Sender<TextDocumentIdentifier>
		) -> (Url, Arc<Workspace<T>>)
		{
			(workspace_folder.uri.clone(), Arc::new(Workspace::new(file_system, workspace_folder, index_sender)))
		}

		let (has_workspaces, workspace_folders) = workspace_folders
			.map_or_else(
				|| { (false, vec![WorkspaceFolder{ name: "<*>".to_string(), uri: Url::parse("file:///").unwrap() }]) },
				|folders| { (true, folders) });

		let (index_sender, index_receiver) = async_channel::unbounded::<TextDocumentIdentifier>();

		AsyncPool::run_as_task(index_files(index_receiver.clone(), file_system.clone())).unwrap();

		Self {
			has_workspaces,
			workspaces: workspace_folders.into_iter().map(|wf| to_workspace(file_system.clone(), wf, index_sender.clone())).collect()
		}
	}

	/// Returns `true` if the [`WorkspaceManager`] was initialized with workspace folders; otherwise `false`.
	pub fn has_workspaces(&self) -> bool {
		self.has_workspaces
	}

	/// Retrieves a file from a workspace.
	///
	/// [`WorkspaceManager::get_file`] will always return a [`File`] for `uri`. It does this because requests for
	/// files may be made in contexts in which no workspace folders were opened. If this is the case, then the file will
	/// be retrieved relative to the 'catch-all' workspace which is not indexed.
	///
	/// The overall state of the file can be determined from its [`File::get_compiled_unit`] method which will
	/// inform its final state.
	pub fn get_file(&self, uri: Url) -> Arc<File<T>> {
		fn is_descendant_path(base: &Url, target: &Url) -> bool {
			if let Some(relative) = base.make_relative(target) {
				return !relative.starts_with("..");
			}

			false
		}

		// If not initialized with any workspace folders, then the path should always be a descendant of the
		// 'catch-all' workspace.
		match (&self.workspaces).into_iter().find(|(workspace_uri, _)| is_descendant_path(&workspace_uri, &uri)) {
			Some((_, workspace)) => workspace.get_file(uri),
			None => {
				error!(file_uri = uri.as_str(), "Failed to locate a workspace for a given file.");
				unreachable!("failed to locate a workspace");
			}
		}
	}

	/// Asynchronously indexes the contents of each [`Workspace`].
	///
	/// Returns immediately if the the [`WorkspaceManager`] was not initialized with workspace folders.
	pub async fn index(&self, progress: &ProgressManager) {
		if !self.has_workspaces() {
			return; // Do nothing if there are no workspace folders.
		}

		let progress = progress.begin("Indexing").await.unwrap();

		for (_, workspace) in (&self.workspaces).into_iter() {
			progress.report(&format!("{}", workspace)).await.unwrap();

			workspace.index().await;
		}

		progress.end(None).await.unwrap();
	}
}

impl<T: Clone> IntoIterator for WorkspaceManager<T> {
	type Item = (Url, Arc<Workspace<T>>);
	type IntoIter = IntoIter<Url, Arc<Workspace<T>>>;

	/// Creates a consuming iterator of [`Workspace`].
	fn into_iter(self) -> Self::IntoIter {
		self.workspaces.into_iter()
	}
}

impl<'a, T: Clone> IntoIterator for &'a WorkspaceManager<T> {
	type Item = (&'a Url, &'a Arc<Workspace<T>>);
	type IntoIter = Iter<'a, Url, Arc<Workspace<T>>>;

	/// Creates a consuming iterator of &[`Workspace`].
	fn into_iter(self) -> Self::IntoIter {
		self.workspaces.iter()
	}
}

/// Encapsulates a collection of related files opened as part of a set managed by an LSP compliant host.
#[derive(Clone)]
pub(crate) struct Workspace<T: Clone> {
	file_system: Arc<AnyEnumerableFileSystem>,
	workspace_folder: WorkspaceFolder,
	files: Arc<RwLock<HashMap<Url, Arc<File<T>>>>>,
	index_sender: Sender<TextDocumentIdentifier>
}

impl<T> Workspace<T>
where
	T: Clone + Debug
{
	/// Initializes a new [`Workspace`].
	pub fn new(file_system: Arc<AnyEnumerableFileSystem>, workspace_folder: WorkspaceFolder, index_sender: Sender<TextDocumentIdentifier>) -> Self {
		Self {
			file_system,
			workspace_folder,
			files: Arc::new(RwLock::new(HashMap::new())),
			index_sender
		}
	}

	/// Gets the URL of the current [`Workspace`].
	pub fn uri(&self) -> Url {
		self.workspace_folder.uri.clone()
	}

	/// Gets the name of the current [`Workspace`].
	pub fn name(&self) -> &str {
		self.workspace_folder.name.as_str()
	}

	/// Look up and retrieve a file from the workspace.
	///
	/// The [`File`] will be created if it is not present in the current workspace.
	pub fn get_file(&self, uri: Url) -> Arc<File<T>> {
		let mut files = self.files.write().unwrap();
		let workspace_uri = self.uri();
		let new_uri = uri.clone();

		match files.entry(uri) {
			Entry::Occupied(entry) => entry.get().clone(),
			Entry::Vacant(entry) => {
				info!(workspace_uri = workspace_uri.as_str(), file_uri = new_uri.as_str(), "Missing file entry in workspace'{}'.", self.name());

				let new_document_identifier = TextDocumentIdentifier { uri: new_uri };
				let new_file = Arc::new(File::<T>::new(new_document_identifier.clone()));

				entry.insert(new_file.clone());

				// self.index_sender.send_blocking(new_document_identifier).unwrap();

				new_file
			}
		}
	}

	pub async fn index(&self) {
		fn write_files<T: Clone + Debug>(s: &Workspace<T>, document_identifiers: &Vec<TextDocumentIdentifier>) {
			let mut files = s.files.write().unwrap();

			for document_identifier in document_identifiers.into_iter() {
				files.insert(document_identifier.uri.clone(), Arc::new(File::<T>::new(document_identifier.clone())));

				// s.index_sender.send_blocking(document_identifier.clone()).unwrap();
			}
		}

		let document_identifiers = self.file_system.enumerate_folder(self.uri()).await;

		if document_identifiers.len() == 0 {
			return;
		}

		write_files(self, &document_identifiers);
	}
}

impl<T: Clone> Display for Workspace<T> {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "[{}]({})", self.workspace_folder.name, self.workspace_folder.uri)?;

		Ok(())
	}
}

#[derive(Error, Debug, PartialEq, Eq, Clone, Copy)]
pub enum IndexError {
	#[error("An unexpected error occurred during file indexing.")]
	Unexpected
}

#[derive(Clone)]
struct FileState<T: Clone> {
	buffer: Option<String>,
	compiled_unit: FutureCompletionSource<Box<T>, IndexError>
}

#[derive(Clone)]
pub(crate) struct File<T: Clone> {
	document_identifier: TextDocumentIdentifier,
	state: Arc<RwLock<FileState<T>>>
}

impl<T> File<T>
where
	T: Clone + Debug
{
	pub fn new(document_identifier: TextDocumentIdentifier) -> Self {
		Self {
			document_identifier,
			state: Arc::new(RwLock::new(FileState {
				buffer: None,
				compiled_unit: FutureCompletionSource::<Box<T>, IndexError>::new()
			}))
		}
	}

	/// Returns `true` if the current file has a buffer open and under the control of an LSP compliant host.
	pub fn is_open_in_ide(&self) -> bool {
		let state = self.state.read().unwrap();

		state.buffer == None
	}

	/// Returns the current buffer.
	///
	/// Returns [`None`] if the file has no buffer (indicating that the file is not open).
	pub fn current_buffer(&self) -> Option<String> {
		let state = self.state.read().unwrap();

		state.buffer.clone()
	}

	pub async fn get_compiled_unit(&self) -> Result<T, IndexError> {
		let state = self.state.read().unwrap();

		match state.compiled_unit.future().await {
			Ok(boxed_value) => {
				Ok(*boxed_value.clone())
			},
			Err(err) => Err(err)
		}
	}

	pub fn open_or_change_buffer(&self, buffer: String, compiled_unit: T) {
		let mut state = self.state.write().unwrap();

		state.buffer.replace(buffer);

		if let Poll::Ready(result) = state.compiled_unit.state() {
			match result {
				Ok(mut boxed_value) => *boxed_value = compiled_unit,
				Err(_) => state.compiled_unit = FutureCompletionSource::<Box<T>, IndexError>::new_with_value(Box::new(compiled_unit))
			}
		}
		else {
			state.compiled_unit.set_value(Box::new(compiled_unit)).expect("");
		}
	}

	pub fn close_buffer(&self) {
		let mut state = self.state.write().unwrap();

		state.buffer = None;
	}
}

impl<T: Clone> Display for File<T> {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "({})", self.document_identifier.uri)?;

		Ok(())
	}
}
