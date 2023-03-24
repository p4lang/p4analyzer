use crate::tracing::TraceValueAccessor;

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
}

impl Default for State {
	/// Initializes a new default [`State`] instance.
	fn default() -> Self {
		Self {
			trace_value: None,
			analyzer: AnalyzerWrapper(Default::default()).into(),
		}
	}
}
