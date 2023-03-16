use std::{sync::Arc, collections::{HashMap, hash_map::{Iter, IntoIter}}, fmt::{Formatter, Display, Result as FmtResult}, marker::PhantomData};

use analyzer_abstractions::{lsp_types::{WorkspaceFolder, Url}, fs::AnyEnumerableFileSystem};
use analyzer_abstractions::futures_extensions::FutureCompletionSource;

#[derive(Clone)]
pub(crate) struct WorkspaceManager<T: Clone = ()> {
	has_workspaces: bool,
	workspaces: HashMap<Url, Arc<Workspace<T>>>
}

impl<T: Clone> WorkspaceManager<T> {
	/// Initializes a new [`WorkspaceManager`] instance.
	///
	/// If `workspace_folders` is [`None`], then a root workspace folder will be used by default.
	pub fn new(file_system: Arc<AnyEnumerableFileSystem>, workspace_folders: Option<Vec<WorkspaceFolder>>) -> Self {
		fn to_workspace<T: Clone>(file_system: Arc<AnyEnumerableFileSystem>, workspace_folder: WorkspaceFolder) -> (Url, Arc<Workspace<T>>) {
			(workspace_folder.uri.clone(), Arc::new(Workspace::new(file_system, workspace_folder)))
		}

		let (has_workspaces, workspace_folders) = workspace_folders
			.map_or_else(
				|| { (false, vec![WorkspaceFolder{ name: "<*>".to_string(), uri: Url::parse("file:///").unwrap() }]) },
				|folders| { (true, folders) });

		Self {
			has_workspaces,
			workspaces: workspace_folders.into_iter().map(|wf| to_workspace(file_system.clone(), wf)).collect(),
		}
	}

	/// Returns `true` if the [`WorkspaceManager`] was initialized with workspace folders; otherwise `false`.
	pub fn has_workspaces(&self) -> bool {
		self.has_workspaces
	}
}

impl<T: Clone> IntoIterator for WorkspaceManager<T> {
	type Item = (Url, Arc<Workspace<T>>);
	type IntoIter = IntoIter<Url, Arc<Workspace<T>>>;

	fn into_iter(self) -> Self::IntoIter {
		self.workspaces.into_iter()
	}
}

impl<'a, T: Clone> IntoIterator for &'a WorkspaceManager<T> {
	type Item = (&'a Url, &'a Arc<Workspace<T>>);
	type IntoIter = Iter<'a, Url, Arc<Workspace<T>>>;

    fn into_iter(self) -> Self::IntoIter {
			self.workspaces.iter()
    }
}

#[derive(Clone)]
pub(crate) struct Workspace<T: Clone> {
	file_system: Arc<AnyEnumerableFileSystem>,
	workspace_folder: WorkspaceFolder,
	files: HashMap<usize, Arc<File<T>>>
}

impl<T: Clone> Workspace<T> {
	pub fn new(file_system: Arc<AnyEnumerableFileSystem>, workspace_folder: WorkspaceFolder) -> Self {
		Self {
			file_system,
			workspace_folder,
			files: HashMap::new()
		}
	}

	pub fn uri(&self) -> Url {
		self.workspace_folder.uri.clone()
	}

	pub fn name(&self) -> &str {
		self.workspace_folder.name.as_str()
	}
}

impl<T: Clone> Display for Workspace<T> {
	/// Formats a [`Workspace`] using the given formatter.
	fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
		write!(formatter, "[{}]({})", self.workspace_folder.name, self.workspace_folder.uri)?;

		Ok(())
	}
}

#[derive(Clone)]
pub(crate) struct File<T: Clone> {
	file_id: usize,
	buffer: Option<String>,
	compiled_unit: FutureCompletionSource<T, usize>
}
