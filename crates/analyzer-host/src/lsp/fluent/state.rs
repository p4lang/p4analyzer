use crate::lsp::{
	dispatch::DispatchTarget,
	state::{LspServerState, LspTransitionTarget},
};

/// Provides a fluent API for setting state transition options during the registation of a handler for
/// a given message.
pub(crate) struct TransitionBuilder<'target, TState>
where
	TState: Send + Sync,
{
	target: &'target mut dyn DispatchTarget<TState>,
}

impl<'target, TState> TransitionBuilder<'target, TState>
where
	TState: Send + Sync,
{
	/// Initializes a new [`TransitionBuilder`] for a given [`DispatchTarget`].
	pub(crate) fn new(target: &'target mut dyn DispatchTarget<TState>) -> Self {
		Self { target }
	}

	/// Sets the target [`LspServerState`].
	///
	/// If the handler returns [`Ok`], then the specified [`LspServerState`] will be used to indicate that
	/// future messages should be processed in that context. Otherwise the current [`LspServerState`] will
	/// be used instead.
	pub fn transition_to(&mut self, target_state: LspServerState) {
		self.target
			.set_transition_target(LspTransitionTarget::Next(target_state));
	}
}
