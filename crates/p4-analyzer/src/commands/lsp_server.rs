use crate::stdio::ConsoleDriver;
use crate::{Command, CommandInvocationError};
use analyzer_abstractions::Logger;
use analyzer_host::AnalyzerHost;
use async_trait::async_trait;
use cancellation::CancellationToken;

/// A P4 Analyzer command that starts the Language Server Protocol (LSP) server implementation.
pub struct LspServerCommand {}

impl LspServerCommand {
	/// Initializes a new [`LspServerCommand`] instance.
	pub fn new() -> Self {
		LspServerCommand {}
	}
}

#[async_trait]
impl Command for LspServerCommand {
	/// Runs the command by delegating to a P4 Analyzer Host.
	async fn run(&self, cancel_token: &CancellationToken) -> Result<(), CommandInvocationError> {
		let console = ConsoleDriver::new();
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
