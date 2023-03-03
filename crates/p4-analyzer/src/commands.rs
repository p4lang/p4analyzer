use std::sync::Arc;

use analyzer_abstractions::{async_trait::async_trait, tracing::Subscriber};
use analyzer_host::tracing::tracing_subscriber::{Layer, registry::LookupSpan};
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

	/// Retrieves any additional '`Tracing`' logging layers that should be used when running the command.
	///
	/// The [`Command`] trait provides a default implementation that returns an empty [`Vec`]. Implementations should override
	/// this to supply additional logging layers that are required when invocated.
	fn logging_layers<S>(&self) -> Vec<Box<dyn Layer<S> + Send + Sync + 'static>>
	where
		S: Subscriber,
		for<'a> S: LookupSpan<'a>
	{
		vec![] // Return an empty vector by default.
	}
}
