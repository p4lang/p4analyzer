/// Represents the distinct state of a Language Server Protocol (LSP) server implementation.
#[derive(Debug, Eq, Hash, PartialEq, PartialOrd, Copy, Clone)]
pub(crate) enum LspServerState {
	/// The server is active, but not yet initialized.
	ActiveUninitialized,

	/// The server is initializing. It has responded to an 'initialize' request and is now
	/// waiting on the client.
	Initializing,

	/// The server is active, and initialized. It can now accept document synchronization requests.
	ActiveInitialized,

	/// The server is shutting down.
	ShuttingDown,

	/// The server is stopped.
	Stopped,
}

/// Captures a desired change in state.
#[derive(Debug, PartialEq, PartialOrd, Copy, Clone)]
pub(crate) enum LspTransitionTarget {
	/// The state should remain current.
	Current,

	/// The state should transition to a defined 'next' state.
	Next(LspServerState),
}
