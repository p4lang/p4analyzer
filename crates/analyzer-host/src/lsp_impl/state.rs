use std::{fmt, sync::{Arc, atomic::AtomicI32}, collections::HashMap, cell::RefCell};

use analyzer_abstractions::lsp_types::request::Request;
use serde::{de::DeserializeOwned, Serialize};

use crate::{tracing::TraceValueAccessor, json_rpc::{message::Message, RequestId}};

pub(crate) struct AnalyzerWrapper(std::cell::RefCell<analyzer_core::Analyzer>);

unsafe impl Sync for AnalyzerWrapper {}
unsafe impl Send for AnalyzerWrapper {}

impl AnalyzerWrapper {
	pub fn unwrap(&self) -> std::cell::RefMut<analyzer_core::Analyzer> {
		self.0.borrow_mut()
	}
}

/// Represents the active state of the P4 Analyzer.
// #[derive(Clone)]
pub(crate) struct State {
	/// The optional [`TraceValueAccessor`] that can be used to set the trace value used in the LSP tracing layer.
	pub trace_value: Option<TraceValueAccessor>,
	pub analyzer: std::sync::Arc<AnalyzerWrapper>,

	pub request_manager: RequestManager,
}

impl Default for State {
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
		Self {
			// request_id: AtomicI32::new(0),
			active_requests: Arc::new(HashMap::new())
		}
	}
}
