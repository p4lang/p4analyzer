use crate::{tracing::TraceValueAccessor, lsp::request::RequestManager};

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

	/// The [`RequestManager`] instance to use when managing server to client requests.
	pub request_manager: RequestManager,
}

impl State {
	/// Initializes a new default [`State`] instance.
	fn default() -> Self {
		Self {
			trace_value: None,
			analyzer: AnalyzerWrapper(Default::default()).into(),
			request_manager: RequestManager::default(),
		}
	}
}

// #[derive(Clone)]
pub(crate) struct RequestManager {
	// request_id: AtomicI32,
	active_requests: Arc<HashMap<RequestId, Message>>
}

impl RequestManager {
	pub async fn send<T>(&self, params: T::Params) -> ()
	where
		T: Request + 'static,
		T::Params: Clone + DeserializeOwned + Send + fmt::Debug,
		T::Result: Clone + Serialize + Send,
	{

	}
}

impl Default for RequestManager {
	fn default() -> Self {
	pub fn new(trace_value: Option<TraceValueAccessor>, request_manager: RequestManager) -> Self {
		Self {
			trace_value,
			request_manager,
		}
	}
}
