use crate::cli::flags::Server;
use crate::stdio::ConsoleDriver;
use crate::{Command, CommandInvocationError};
use analyzer_abstractions::{tracing::subscriber, Logger};
use analyzer_host::tracing::{
	tracing_subscriber::{fmt::layer, prelude::*, Registry},
	LspTracingLayer,
};
use analyzer_host::AnalyzerHost;
use async_trait::async_trait;
use cancellation::CancellationToken;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// A P4 Analyzer command that starts the Language Server Protocol (LSP) server implementation.
pub struct LspServerCommand {
	config: Server,
}

impl LspServerCommand {
	/// Initializes a new [`LspServerCommand`] instance.
	pub fn new(config: Server) -> Self {
		LspServerCommand { config }
	}
}

#[async_trait]
impl Command for LspServerCommand {
	/// Runs the command by delegating to a P4 Analyzer Host.
	async fn run(&self, cancel_token: &CancellationToken) -> Result<(), CommandInvocationError> {
		let console = ConsoleDriver::new();
		// TODO: Configure the rolling file appender layer using command configuration.
		let trace_appender = RollingFileAppender::new(Rotation::NEVER, ".", "p4-analyzer.log");
		let (non_blocking, _guard) = tracing_appender::non_blocking(trace_appender);
		let layer = layer().with_writer(non_blocking);

		let subscriber = Registry::default()
			.with(layer)
			.with(LspTracingLayer::new(console.get_message_channel()));

		subscriber::set_global_default(subscriber)
			.expect("Unable to set global tracing subscriber.");

		let host = AnalyzerHost::new(console.get_message_channel(), &ConsoleLogger {});

		match tokio::join!(host.start(cancel_token), console.start(cancel_token)) {
			(Ok(_), Ok(_)) => Ok(()),
			_ => Err(CommandInvocationError::Cancelled),
		}
	}
}

struct ConsoleLogger {}

impl Logger for ConsoleLogger {
	fn log_message(&self, msg: &str) {
		println!("{}", msg);
	}

	fn log_error(&self, msg: &str) {
		eprintln!("{}", msg);
	}
}
