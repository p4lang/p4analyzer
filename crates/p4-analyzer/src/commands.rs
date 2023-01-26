use std::sync::Arc;

use async_trait::async_trait;
use cancellation::CancellationToken;
use thiserror::Error;

pub(crate) mod lsp_server;

/// Defines a command invocation error.
#[derive(Error, Debug)]
pub enum CommandInvocationError {
	// The command was cancelled.
	#[error("The command was cancelled.")]
	Cancelled,

	/// An unexpected error.
	#[error("An unexpected error occurred executing the command.")]
	Unknown,
}

/// A P4 Analyzer command.
#[async_trait]
pub(crate) trait Command {
	/// Runs the command.
	async fn run(&self, cancel_token: Arc<CancellationToken>) -> Result<(), CommandInvocationError>;
}
