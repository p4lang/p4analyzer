// Export the Language Server Protocol types.
pub use lsp_types;
pub use tracing;

pub trait Logger {
	fn log_message(&self, msg: &str);

	fn log_error(&self, msg: &str);
}

pub type LoggerImpl = dyn Logger + Send + Sync;
