use std::sync::Arc;

use analyzer_abstractions::{
	fs::AnyEnumerableFileSystem,
	futures::future::join_all,
	futures_extensions::async_extensions::AsyncPool,
	lsp_types::{TraceValue, Url},
	tracing::info,
};
use analyzer_core::base_abstractions::{FileId, IncludedDependency};
use async_channel::{Receiver, Sender};

use crate::{
	lsp::{
		analyzer::{Analyzer, AnyAnalyzer},
		progress::{Progress, ProgressManager},
		request::RequestManager,
		workspace::{File, IndexError, WorkspaceManager},
		LspProtocolError,
	},
	tracing::TraceValueAccessor,
};

pub(crate) struct AnalyzerWrapper {
	inner: std::cell::RefCell<analyzer_core::Analyzer>,
	background_channel: Sender<Arc<File>>,
}

unsafe impl Sync for AnalyzerWrapper {}
unsafe impl Send for AnalyzerWrapper {}

impl AnalyzerWrapper {
	pub fn new(background_channel: Sender<Arc<File>>) -> Self { Self { inner: Default::default(), background_channel } }
}

impl Analyzer for AnalyzerWrapper {
	fn unwrap(&self) -> std::cell::RefMut<analyzer_core::Analyzer> { self.inner.borrow_mut() }

	fn background_analyze(&self, file: Arc<File>) {
		// Enqueue the received file onto the background channel for processing.
		self.background_channel.send_blocking(file.clone()).unwrap();
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

	/// A [`FileChannel`] where sent files will be parsed in the background.
	background_parse_channel: (Sender<Arc<File>>, Receiver<Arc<File>>),
}

impl State {
	/// Initializes a new [`State`] instance.
	pub fn new(
		trace_value: Option<TraceValueAccessor>,
		request_manager: RequestManager,
		file_system: Arc<AnyEnumerableFileSystem>,
	) -> Self {
		let background_parse_channel = async_channel::unbounded::<Arc<File>>();
		let (sender, _) = background_parse_channel.clone();

		Self {
			trace_value,
			analyzer: Arc::new(Box::new(AnalyzerWrapper::new(sender))),
			file_system,
			request_manager,
			progress_manager: None,
			workspace_manager: None,
			background_parse_channel,
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
	pub fn has_workspaces(&self) -> bool { self.workspaces().has_workspaces() }

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
		self.workspace_manager.replace(workspace_manager);

		// With the WorkspaceManager now in place, we can now also start processing background analyze requests for files.
		let (_, receiver) = self.background_parse_channel.clone();

		AsyncPool::spawn_work(State::process_background_analyze_requests(
			receiver,
			self.file_system.clone(),
			self.workspaces().clone(),
			self.analyzer.clone(),
		));
	}

	/// Sets the current [`ProgressManager`] for the current instance of the P4 Analyzer.
	///
	/// This method should be invoked when processing the
	/// [`'initialize'`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#initialize)
	/// request from the LSP client.
	pub(crate) fn set_progress(&mut self, progress_manager: ProgressManager) {
		self.progress_manager = Some(progress_manager);
	}

	async fn process_background_analyze_requests(
		receiver: Receiver<Arc<File>>,
		file_system: Arc<AnyEnumerableFileSystem>,
		workspace_manager: WorkspaceManager,
		analyzer: Arc<AnyAnalyzer>,
	) {
		fn analyze_source_text(analyzer: &AnyAnalyzer, uri: &str, text: String) -> (FileId, Vec<IncludedDependency>) {
			let mut analyzer = analyzer.unwrap();
			let file_id = analyzer.file_id(uri);

			analyzer.update(file_id, text);
			analyzer.preprocessed(file_id);

			(file_id, analyzer.include_dependencies(file_id))
		}

		loop {
			match receiver.recv().await {
				Ok(file) => {
					info!(file_uri = file.document_identifier.uri.as_str(), "Background parsing");

					// If the file has been opened in the IDE during the time taken to start this background
					// analyze, then simply ignore it. The IDE is now the source of truth for this file.
					if file.is_open_in_ide() {
						continue;
					}

					match file_system.file_contents(file.document_identifier.uri.clone()).await {
						Some(text) => {
							info!(file_uri = file.document_identifier.uri.as_str(), "Got text: {}", text);

							// If the file has been opened in the IDE during the fetching of its contents, then simply
							// throw it all away. The IDE is now the source of truth for this file. Otherwise, update its
							// parsed unit.
							if !file.is_open_in_ide() {
								let (file_id, file_includes) =
									analyze_source_text(&analyzer, file.document_identifier.uri.as_str(), text);

								if !file_includes.is_empty() {
									let file_include_resolvers = file_includes.iter().map(|include| async {
										let file =
											workspace_manager.get_file(Url::parse(&include.include_path).unwrap());

										file.get_parsed_unit().await
									});

									join_all(file_include_resolvers).await;
								}

								file.set_parsed_unit(file_id, None);
							}
						}
						None => file.set_index_error(IndexError::NotFound), // The file was not found
					}
				}
				Err(_) => break,
			}
		}
	}
}
