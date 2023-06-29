use async_rwlock::RwLock as AsyncRwLock;
use core::fmt::Debug;
use std::{
	collections::{
		hash_map::{Entry, Iter},
		HashMap,
	},
	fmt::{Display, Formatter, Result as FmtResult},
	sync::{Arc, RwLock},
	task::Poll,
};

use crate::lsp::RELATIVE_P4_SOURCEFILES_GLOBPATTERN;
use analyzer_abstractions::{
	fs::AnyEnumerableFileSystem,
	futures_extensions::FutureCompletionSource,
	lsp_types::{TextDocumentIdentifier, Url, WorkspaceFolder},
	tracing::{error, info},
};
use thiserror::Error;

use super::{
	analyzer::{AnyBackgroundLoad, ParsedUnit},
	progress::ProgressManager,
};

/// Manages a collection of workspaces opened by an LSP compliant host.
#[derive(Clone)]
pub struct WorkspaceManager {
	has_workspaces: bool,
	pub workspaces: HashMap<Url, Arc<Workspace>>,
}

impl WorkspaceManager {
	/// Initializes a new [`WorkspaceManager`] instance.
	///
	/// If `workspace_folders` is [`None`], then a root workspace folder will be used by default.
	pub fn new(
		file_system: Arc<AnyEnumerableFileSystem>,
		workspace_folders: Option<Vec<WorkspaceFolder>>,
		background_load: Arc<AnyBackgroundLoad>,
	) -> Self {
		fn to_workspace(
			file_system: Arc<AnyEnumerableFileSystem>,
			workspace_folder: WorkspaceFolder,
			background_load: Arc<AnyBackgroundLoad>,
		) -> (Url, Arc<Workspace>) {
			(
				workspace_folder.uri.clone(),
				Arc::new(Workspace::new(file_system, workspace_folder, background_load)),
			)
		}

		let (has_workspaces, workspace_folders) = workspace_folders.map_or_else(
			|| (false, vec![WorkspaceFolder { name: "<*>".to_string(), uri: Url::parse("file:///").unwrap() }]),
			|folders| (true, folders),
		);

		Self {
			has_workspaces,
			workspaces: workspace_folders
				.into_iter()
				.map(|wf| to_workspace(file_system.clone(), wf, background_load.clone()))
				.collect(),
		}
	}

	/// Returns `true` if the [`WorkspaceManager`] was initialized with workspace folders; otherwise `false`.
	pub fn has_workspaces(&self) -> bool { self.has_workspaces }

	/// Retrieves a file from a workspace.
	///
	/// [`WorkspaceManager::get_file`] will always return a [`File`] for `uri`. It does this because requests for
	/// files may be made in contexts in which no workspace folders were opened. If this is the case, then the file will
	/// be retrieved relative to the 'catch-all' workspace which is not indexed.
	///
	/// The overall state of the file can be determined from its [`File::get_parsed_unit`] method which will
	/// inform its final state.
	pub fn get_file(&self, uri: Url) -> Arc<File> {
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
	/// Returns immediately if the [`WorkspaceManager`] was not initialized with workspace folders.
	pub(crate) async fn index(&self, progress: &ProgressManager) {
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

impl<'a> IntoIterator for &'a WorkspaceManager {
	type Item = (&'a Url, &'a Arc<Workspace>);
	type IntoIter = Iter<'a, Url, Arc<Workspace>>;

	/// Creates a consuming iterator of &[`Workspace`].
	fn into_iter(self) -> Self::IntoIter { self.workspaces.iter() }
}

/// Encapsulates a collection of related files opened as part of a set managed by an LSP compliant host.
#[derive(Clone)]
pub struct Workspace {
	file_system: Arc<AnyEnumerableFileSystem>,
	workspace_folder: WorkspaceFolder,
	background_load: Arc<AnyBackgroundLoad>,
	pub files: Arc<RwLock<HashMap<Url, Arc<File>>>>,
}

impl Workspace {
	/// Initializes a new [`Workspace`].
	pub fn new(
		file_system: Arc<AnyEnumerableFileSystem>,
		workspace_folder: WorkspaceFolder,
		background_load: Arc<AnyBackgroundLoad>,
	) -> Self {
		Self { file_system, workspace_folder, background_load, files: Arc::new(RwLock::new(HashMap::new())) }
	}

	/// Gets the URL of the current [`Workspace`].
	pub fn uri(&self) -> Url { self.workspace_folder.uri.clone() }

	/// Gets the name of the current [`Workspace`].
	pub fn name(&self) -> &str { self.workspace_folder.name.as_str() }

	/// Look up and retrieve a file from the workspace.
	///
	/// The [`File`] will be created if it is not present in the current workspace.
	pub fn get_file(&self, uri: Url) -> Arc<File> {
		let mut files = self.files.write().unwrap();

		match files.entry(uri.clone()) {
			Entry::Occupied(entry) => entry.get().clone(),
			Entry::Vacant(entry) => {
				let new_document_identifier = TextDocumentIdentifier { uri: uri.clone() };
				let new_file = Arc::new(File::new(new_document_identifier.clone()));

				entry.insert(new_file.clone());

				// The file was likely never indexed (i,e., no workspace folders were opened), so
				// background load the file.
				self.background_load.load(uri.clone());

				new_file
			}
		}
	}

	pub async fn index(&self) {
		fn write_files(workspace: &Workspace, document_identifiers: &Vec<TextDocumentIdentifier>) {
			let mut files = workspace.files.write().unwrap();

			for document_identifier in document_identifiers.into_iter() {
				files.insert(document_identifier.uri.clone(), Arc::new(File::new(document_identifier.clone())));

				// Background load the file now that its known to the Workspace.
				workspace.background_load.load(document_identifier.uri.clone());
			}
		}

		let document_identifiers =
			self.file_system.enumerate_folder(self.uri(), RELATIVE_P4_SOURCEFILES_GLOBPATTERN.into()).await;

		info!(
			workspace_uri = self.uri().as_str(),
			document_count = document_identifiers.len(),
			"Workspace indexing complete."
		);

		if document_identifiers.len() == 0 {
			return;
		}

		write_files(self, &document_identifiers);
	}
}

impl Display for Workspace {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "[{}]({})", self.workspace_folder.name, self.workspace_folder.uri)?;

		Ok(())
	}
}

#[derive(Error, Debug, PartialEq, Eq, Clone, Copy)]
pub enum IndexError {
	#[error("An unexpected error occurred during file indexing.")]
	Unexpected,

	#[error("The file could not be found.")]
	NotFound,
}

#[derive(Clone)]
pub(crate) struct FileState {
	is_open_in_ide: bool,
	parsed_unit: FutureCompletionSource<Arc<RwLock<ParsedUnit>>, IndexError>,
}

unsafe impl Sync for FileState {}
unsafe impl Send for FileState {}

#[derive(Clone)]
pub struct File {
	pub document_identifier: TextDocumentIdentifier,
	state: Arc<AsyncRwLock<FileState>>,
}

impl File {
	pub fn new(document_identifier: TextDocumentIdentifier) -> Self {
		Self {
			document_identifier,
			state: Arc::new(AsyncRwLock::new(FileState {
				is_open_in_ide: false,
				parsed_unit: FutureCompletionSource::<Arc<RwLock<ParsedUnit>>, IndexError>::new(),
			})),
		}
	}

	/// Returns `true` if the current file has a buffer open and under the control of an LSP compliant host.
	pub fn is_open_in_ide(&self) -> bool {
		let state = self.state.try_read().unwrap();

		state.is_open_in_ide
	}

	pub async fn get_parsed_unit(&self) -> Result<ParsedUnit, IndexError> {
		let state = self.state.read().await;
		let parsed_unit = state.parsed_unit.clone();

		// Release the read-lock on state before awaiting on the parsed unit.
		drop(state);

		match parsed_unit.future().await {
			Ok(current_parsed_unit) => Ok(current_parsed_unit.read().unwrap().clone()),
			Err(err) => Err(err),
		}
	}

	pub fn open_or_update(&self, parsed_unit: ParsedUnit) {
		let mut state = self.state.try_write().unwrap();

		state.is_open_in_ide = true;

		self.set_parsed_unit(parsed_unit, Some(state)); // Use the writable state that we already have.
	}

	pub fn close(&self) {
		let mut state = self.state.try_write().unwrap();

		state.is_open_in_ide = false;
	}

	pub(crate) fn set_parsed_unit(
		&self,
		parsed_unit: ParsedUnit,
		state: Option<async_rwlock::RwLockWriteGuard<FileState>>,
	) {
		let mut state = state.unwrap_or_else(|| self.state.try_write().unwrap());

		// If the state of the FutureCompletionSource holding a parsed unit is 'Ready', then it means that
		// we are already in receipt of output from the Analyzer (i.e., from a background fetch and analyze). In this
		// case, we simply want to the update its value. If the FutureCompletionSource completed with an error,
		// then we need to replace it with a new FutureCompletionSource instance that has the supplied parsed unit
		// set for it.
		//
		// If the FutureCompletionSource is still pending, then we can simply 'complete' it with the given parsed unit.
		if let Poll::Ready(result) = state.parsed_unit.state() {
			match result {
				Ok(current_parsed_unit) => *current_parsed_unit.write().unwrap() = parsed_unit,
				Err(_) => state.parsed_unit = FutureCompletionSource::new_with_value(RwLock::new(parsed_unit).into()),
			}
		} else {
			state.parsed_unit.set_value(RwLock::new(parsed_unit).into()).unwrap();
		}
	}

	pub(crate) fn set_index_error(&self, error: IndexError) {
		let mut state = self.state.try_write().unwrap();

		// If the state of the FutureCompletionSource holding a parsed unit is 'Ready', then replace it with one
		// that has the given error set on it.
		//
		// If the FutureCompletionSource is still pending, then we can simply 'complete' it with the given error.
		if let Poll::Ready(_) = state.parsed_unit.state() {
			state.parsed_unit = {
				let fcs = FutureCompletionSource::new();

				fcs.set_err(error).unwrap();

				fcs
			}
		} else {
			state.parsed_unit.set_err(error).unwrap();
		}
	}
}

impl Display for File {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "({})", self.document_identifier.uri)?;

		Ok(())
	}
}
