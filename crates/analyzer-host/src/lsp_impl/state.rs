use crate::tracing::TraceValueAccessor;

/// Represents the active state of the P4 Analyzer.
#[derive(Clone)]
pub(crate) struct State {
	/// The optional [`TraceValueAccessor`] that can be used to set the trace value used in the LSP tracing layer.
	pub trace_value: Option<TraceValueAccessor>,
}

impl Default for State {
	/// Initializes a new default [`State`] instance.
	fn default() -> Self {
		Self {
			trace_value: None
		}
	}
}
