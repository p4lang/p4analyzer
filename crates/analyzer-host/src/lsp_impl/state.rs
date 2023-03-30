use std::{sync::Arc, hash::Hash};

use analyzer_abstractions::{lsp_types::{TraceValue, NumberOrString, TextDocumentIdentifier}, fs::AnyEnumerableFileSystem};
use analyzer_core::base_abstractions::FileId;

use crate::{tracing::TraceValueAccessor, lsp::{request::RequestManager, workspace::WorkspaceManager, progress::{ProgressManager, Progress}, LspProtocolError, analyzer::{Analyzer, AnyAnalyzer}}};

pub(crate) struct AnalyzerWrapper(std::cell::RefCell<analyzer_core::Analyzer>);

unsafe impl Sync for AnalyzerWrapper {}
unsafe impl Send for AnalyzerWrapper {}

impl Analyzer for AnalyzerWrapper {
	fn unwrap(&self) -> std::cell::RefMut<analyzer_core::Analyzer> {
		self.0.borrow_mut()
	}

	fn parse_text_document_contents(&self, document_identifier: TextDocumentIdentifier, contents: String) -> FileId {
		let analyzer = self.unwrap();

		let file_id = analyzer.file_id(document_identifier.uri.as_str());

		file_id
	}
}

/// Represents the active state of the P4 Analyzer.
#[derive(Clone)]
pub(crate) struct State {
	/// The optional [`TraceValueAccessor`] that can be used to set the trace value used in the LSP tracing layer.
	pub trace_value: Option<TraceValueAccessor>,

	/// The Analyzer that will be used to parse and analyze `'.p4'` source files.
	pub analyzer: Arc<AnyAnalyzer>,

	/// The file system that can be used to enumerate folders and retrieve file contents.
	pub file_system: Arc<AnyEnumerableFileSystem>,

	/// The [`RequestManager`] instance to use when sending LSP client requests.
	pub request_manager: RequestManager,

	/// A [`ProgressManager`] instance that can be used to to report work done progress to the LSP client.
	progress_manager: Option<ProgressManager>,

	/// A [`WorkspaceManager`] that can be used to coordinate workspace and file operations.
	workspace_manager: Option<WorkspaceManager>,
}

impl State {
	/// Initializes a new [`State`] instance.
	pub fn new(trace_value: Option<TraceValueAccessor>, request_manager: RequestManager, file_system: Arc<AnyEnumerableFileSystem>) -> Self {
		Self {
			trace_value,
			analyzer: Arc::new(Box::new(AnalyzerWrapper(Default::default()))), // AnalyzerWrapper(Default::default()).into(),
			file_system,
			request_manager,
			progress_manager: None,
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

	/// Returns a reference to the current [`ProgressManager`].
	pub fn progress_manager(&self) -> &ProgressManager {
		if let None = self.progress_manager {
			unreachable!("the ProgressManager was not initialized");
		}

		self.progress_manager.as_ref().unwrap()
	}

	/// Returns a [`Progress`] instance from the initialized Progress Manager that can be used to report
	/// progress to the LSP client.
	pub async fn begin_work_done_progress(&self, title: &str) -> Result<Progress, LspProtocolError> {
		self.progress_manager().begin(title.into()).await
	}

	/// Sets the current [`WorkspaceManager`] for the current instance of the P4 Analyzer.
	///
	/// This method should be invoked when processing the
	/// [`'initialize'`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#initialize)
	/// request from the LSP client.
	pub(crate) fn set_workspaces(&mut self, workspace_manager: WorkspaceManager) {
		self.workspace_manager = Some(workspace_manager);
	}

	/// Sets the current [`ProgressManager`] for the current instance of the P4 Analyzer.
	///
	/// This method should be invoked when processing the
	/// [`'initialize'`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#initialize)
	/// request from the LSP client.
	pub(crate) fn set_progress(&mut self, progress_manager: ProgressManager) {
		self.progress_manager = Some(progress_manager);
	}
}
