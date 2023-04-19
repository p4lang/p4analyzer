use crate::{cli::flags::Server, driver::{Driver, console_driver, DriverType}, Command, CommandInvocationError};
use analyzer_abstractions::{async_trait::async_trait, tracing::Subscriber};
use analyzer_host::{
	tracing::{
		tracing_subscriber::{registry::LookupSpan, Layer},
		LspTracingLayer, TraceValueAccessor,
	},
	AnalyzerHost,
};
use cancellation::CancellationToken;
use std::sync::{Arc, Mutex};

/// A P4 Analyzer command that starts the Language Server Protocol (LSP) server implementation.
pub struct LspServerCommand {
	config: Server,
	driver: Driver,
	trace_value: Mutex<Option<TraceValueAccessor>>,
}

impl LspServerCommand {
	/// Initializes a new [`LspServerCommand`] instance.
	pub fn new(config: Server, driver_type: DriverType) -> Self {
		LspServerCommand { config, driver: Driver::new(driver_type), trace_value: Mutex::new(None) }
	}

	fn trace_value(&self) -> Option<TraceValueAccessor> {
		let trace_value = self.trace_value.lock().unwrap();

		trace_value.clone()
	}
}

#[async_trait]
impl Command for LspServerCommand {
	/// Overrides the default [`Command::logging_layers()`] function to provide a '`Tracing`' logging layer that writes
	/// trace events to the LSP client.
	fn logging_layers<S>(&self) -> Vec<Box<dyn Layer<S> + Send + Sync + 'static>>
	where
		S: Subscriber,
		for<'a> S: LookupSpan<'a>,
	{
		// Create a new `LspTracingLayer` and capture the `TraceValueAccessor` from it before returning
		// its ownership to the caller.
		let layer = LspTracingLayer::new(self.driver.get_message_channel());
		let mut trace_value = self.trace_value.lock().unwrap();

		trace_value.replace(layer.trace_value());

		vec![Box::new(layer)]
	}

	/// Runs the command by delegating to a P4 Analyzer Host.
	async fn run(&self, cancel_token: Arc<CancellationToken>) -> Result<(), CommandInvocationError> {
		// Passing `None` as the `file_system`. This will then default to the LSP based file system that works
		// with the client extensions built as part of the P4 Analyzer Visual Studio Code extension.
		let host = AnalyzerHost::new(self.driver.get_message_channel(), self.trace_value(), None);

		match tokio::join!(host.start(cancel_token.clone()), self.driver.start(cancel_token.clone())) {
			(Ok(_), Ok(_)) => Ok(()),
			_ => Err(CommandInvocationError::Cancelled),
		}
	}
}
