use std::sync::Arc;

use analyzer_abstractions::{lsp_types::TraceValue, fs::AnyEnumerableFileSystem};

use crate::{tracing::TraceValueAccessor, lsp::{request::RequestManager, workspace::WorkspaceManager}};

pub(crate) struct AnalyzerWrapper(std::cell::RefCell<analyzer_core::Analyzer>);

unsafe impl Sync for AnalyzerWrapper {}
unsafe impl Send for AnalyzerWrapper {}

impl AnalyzerWrapper {
	pub fn unwrap(&self) -> std::cell::RefMut<analyzer_core::Analyzer> {
		self.0.borrow_mut()
	}
}

/// Represents the active state of the P4 Analyzer.
#[derive(Clone)]
pub(crate) struct State {
	/// The optional [`TraceValueAccessor`] that can be used to set the trace value used in the LSP tracing layer.
	pub trace_value: Option<TraceValueAccessor>,
	pub analyzer: std::sync::Arc<AnalyzerWrapper>,

	/// The file system that can be used to enumerate folders and retrieve file contents.
	pub file_system: Arc<AnyEnumerableFileSystem>,

	/// The [`RequestManager`] instance to use when sending LSP client requests.
	pub request_manager: RequestManager,

	/// A [`WorkspaceManager`] that can be used to coordinate workspace and file operations.
	workspace_manager: Option<WorkspaceManager>,
}

impl State {
	/// Initializes a new [`State`] instance.
	pub fn new(trace_value: Option<TraceValueAccessor>, request_manager: RequestManager, file_system: Arc<AnyEnumerableFileSystem>) -> Self {
		Self {
			trace_value,
			analyzer: AnalyzerWrapper(Default::default()).into(),
			file_system,
			request_manager,
			workspace_manager: None
		}
	}

	/// If available, sets the supplied [`TraceValue`] on the [`TraceValueAccessor`] thereby modifying the trace
	/// value used by the LSP tracing layer.
	pub fn set_trace_value(&self, value: TraceValue) {
		if let Some(trace_value) = &self.trace_value {
			trace_value.set(value);
		}
	}

	/// Returns `true` if the current P4 Analyzer instance has been started in the context of a workspace.
	pub fn has_workspaces(&self) -> bool {
		self.workspaces().has_workspaces()
	}

	/// Returns a reference to the current [`WorkspaceManager`].
	pub fn workspaces(&self) -> &WorkspaceManager {
		if let None = self.workspace_manager {
			unreachable!("the WorkspaceManager was not initialized"); // A WorkspaceManager should be set during processing of the `'initialize'` request.
		}

		self.workspace_manager.as_ref().unwrap()
	}

	/// Sets the current [`WorkspaceManager`] for the current instance of the P4 Analyzer.
	///
	/// This method should be invoked when processing the
	/// [`'initialize'`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#initialize)
	/// request from the LSP client.
	pub(crate) fn set_workspaces(&mut self, workspace_manager: WorkspaceManager) {
		self.workspace_manager = Some(workspace_manager);
	}
}
