use async_rwlock::RwLock as AsyncRwLock;
use core::fmt::Debug;
use std::{
	collections::{
		hash_map::{Entry, IntoIter, Iter},
		HashMap,
	},
	fmt::{Display, Formatter, Result as FmtResult},
	sync::{Arc, RwLock},
	task::Poll,
};

use crate::lsp::RELATIVE_P4_SOURCEFILES_GLOBPATTERN;
use analyzer_abstractions::{
	fs::AnyEnumerableFileSystem,
	futures_extensions::{async_extensions::AsyncPool, FutureCompletionSource},
	lsp_types::{TextDocumentIdentifier, Url, WorkspaceFolder},
	tracing::{error, info},
};
use async_channel::{Receiver, Sender};
use thiserror::Error;

use super::analyzer::ParsedUnit;

use super::{analyzer::AnyAnalyzer, progress::ProgressManager};

/// Manages a collection of workspaces opened by an LSP compliant host.
#[derive(Clone)]
pub(crate) struct WorkspaceManager {
	has_workspaces: bool,
	workspaces: HashMap<Url, Arc<Workspace>>,
}

impl WorkspaceManager {
	/// Initializes a new [`WorkspaceManager`] instance.
	///
	/// If `workspace_folders` is [`None`], then a root workspace folder will be used by default.
	pub fn new(
		file_system: Arc<AnyEnumerableFileSystem>,
		workspace_folders: Option<Vec<WorkspaceFolder>>,
		analyzer: Arc<AnyAnalyzer>,
	) -> Self {
		fn to_workspace(
			file_system: Arc<AnyEnumerableFileSystem>,
			workspace_folder: WorkspaceFolder,
			analyzer: Arc<AnyAnalyzer>,
		) -> (Url, Arc<Workspace>) {
			(workspace_folder.uri.clone(), Arc::new(Workspace::new(file_system, workspace_folder, analyzer)))
		}

		let (has_workspaces, workspace_folders) = workspace_folders.map_or_else(
			|| (false, vec![WorkspaceFolder { name: "<*>".to_string(), uri: Url::parse("file:///").unwrap() }]),
			|folders| (true, folders),
		);

		Self {
			has_workspaces,
			workspaces: workspace_folders
				.into_iter()
				.map(|wf| to_workspace(file_system.clone(), wf, analyzer.clone()))
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
	/// The overall state of the file can be determined from its [`File::get_compiled_unit`] method which will
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

	pub fn close(&self) {
		for workspace in self.workspaces.clone().into_iter() {
			workspace.1.close();
		}
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
pub(crate) struct Workspace {
	file_system: Arc<AnyEnumerableFileSystem>,
	workspace_folder: WorkspaceFolder,
	files: Arc<RwLock<HashMap<Url, Arc<File>>>>,
	parse_sender: Sender<Arc<File>>,
}

impl Workspace {
	/// Initializes a new [`Workspace`].
	pub fn new(
		file_system: Arc<AnyEnumerableFileSystem>,
		workspace_folder: WorkspaceFolder,
		analyzer: Arc<AnyAnalyzer>,
	) -> Self {
		let (sender, receiver) = async_channel::unbounded::<Arc<File>>();

		AsyncPool::spawn_work(background_analyze(receiver, file_system.clone(), analyzer));

		Self { file_system, workspace_folder, files: Arc::new(RwLock::new(HashMap::new())), parse_sender: sender }
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
		let workspace_uri = self.uri();
		let new_uri = uri.clone();

		match files.entry(uri) {
			Entry::Occupied(entry) => entry.get().clone(),
			Entry::Vacant(entry) => {
				info!(
					workspace_uri = workspace_uri.as_str(),
					file_uri = new_uri.as_str(),
					"Unindexed file in workspace."
				);

				let new_document_identifier = TextDocumentIdentifier { uri: new_uri };
				let new_file = Arc::new(File::new(new_document_identifier.clone()));

				entry.insert(new_file.clone());

				self.parse_sender.send_blocking(new_file.clone()).unwrap();

				new_file
			}
		}
	}

	pub async fn index(&self) {
		fn write_files(s: &Workspace, document_identifiers: &Vec<TextDocumentIdentifier>) {
			let mut files = s.files.write().unwrap();

			for document_identifier in document_identifiers.into_iter() {
				let new_file = Arc::new(File::new(document_identifier.clone()));

				files.insert(document_identifier.uri.clone(), new_file.clone());

				s.parse_sender.send_blocking(new_file.clone()).unwrap();
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

	pub fn close(&self) { self.parse_sender.close(); }
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
}

#[derive(Clone)]
struct FileState {
	is_open_in_ide: bool,
	parsed_unit: FutureCompletionSource<Arc<RwLock<ParsedUnit>>, IndexError>,
}

unsafe impl Sync for FileState {}
unsafe impl Send for FileState {}

#[derive(Clone)]
pub(crate) struct File {
	document_identifier: TextDocumentIdentifier,
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
		let state = self.state.try_read().unwrap();

		match state.parsed_unit.future().await {
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

	fn set_parsed_unit(&self, parsed_unit: ParsedUnit, state: Option<async_rwlock::RwLockWriteGuard<FileState>>) {
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
}

impl Display for File {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "({})", self.document_identifier.uri)?;

		Ok(())
	}
}

async fn background_analyze(
	receiver: Receiver<Arc<File>>,
	file_system: Arc<AnyEnumerableFileSystem>,
	analyzer: Arc<AnyAnalyzer>,
) {
	loop {
		match receiver.recv().await {
			Ok(file) => {
				info!(file_uri = file.document_identifier.uri.as_str(), "Background parsing");

				// If the file has been opened in the IDE during the time taken to start this background parse, then
				// ignore it. The IDE is now the source of truth for this file.
				if file.is_open_in_ide() {
					continue;
				}

				match file_system.file_contents(file.document_identifier.uri.clone()).await {
					Some(contents) => {
						info!(file_uri = file.document_identifier.uri.as_str(), "Got contents: {}", contents);

						// If the file has been opened in the IDE during the fetching of its contents, then
						// throw it all away. The IDE is now the source of truth for this file. Otherwise, update its
						// parsed unit.
						if !file.is_open_in_ide() {
							let parsed_unit =
								analyzer.parse_text_document_contents(file.document_identifier.clone(), contents);

							file.set_parsed_unit(parsed_unit, None);
						}
					}
					None => {
						error!(file_uri = file.document_identifier.uri.as_str(), "Failed to retrieve file contents.");
					}
				}
			}
			Err(_) => break,
		};
	}
}
