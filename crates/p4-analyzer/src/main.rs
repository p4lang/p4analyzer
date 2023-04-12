mod cli;
mod commands;
mod driver;

use analyzer_abstractions::{
	event_listener::Event,
	futures_extensions::async_extensions::AsyncPool,
	tracing::{subscriber, Level, Subscriber},
};
use analyzer_host::tracing::tracing_subscriber::{
	fmt::{layer, writer::MakeWriterExt},
	prelude::__tracing_subscriber_SubscriberExt,
	registry::LookupSpan,
	Layer, Registry,
};
use cancellation::CancellationTokenSource;
use cli::flags::{P4Analyzer, P4AnalyzerCmd};
use commands::{lsp_server::LspServerCommand, Command, CommandInvocationError};
use std::{
	env::current_exe,
	fs, process,
	sync::{
		atomic::{AtomicU8, Ordering},
		Arc,
	},
};
use tracing_appender::{
	non_blocking::WorkerGuard,
	rolling::{RollingFileAppender, Rotation},
};

/// Entry point for the P4 Analyzer.
#[tokio::main]
pub async fn main() {
	match P4Analyzer::from_env() {
		Ok(cmd) => {
			let default_logging_layer = create_default_logging_layer::<Registry>(&cmd);
			let mut layers = if let Some((layer, _)) = default_logging_layer { vec![layer] } else { vec![] };
			let cmd = match cmd.subcommand {
				P4AnalyzerCmd::Server(config) => RunnableCommand(LspServerCommand::new(config)),
				_ => unreachable!(),
			};

			layers.append(&mut cmd.logging_layers());

			let subscriber = Registry::default().with(layers);

			subscriber::set_global_default(subscriber).expect("Unable to set global tracing subscriber.");

			cmd.run().await;
		}
		Err(err) => {
			println!();
			println!("{}", err);
			println!();
		}
	}
}

/// Retrieves the default logging layer based on the presence of the '`--logpath`' CLI argument
fn create_default_logging_layer<S>(cmd: &P4Analyzer) -> Option<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard)>
where
	S: Subscriber,
	for<'a> S: LookupSpan<'a>,
{
	let default_level: String = String::from("debug");
	let logpath = cmd.logpath.as_ref()?;
	let loglevel = cmd.loglevel.as_ref().unwrap_or(&default_level).parse::<Level>().unwrap_or(Level::DEBUG);

	match fs::metadata(logpath) {
		Ok(ref pathinfo) if pathinfo.is_dir() => {
			let file_writer = RollingFileAppender::new(Rotation::NEVER, logpath, format!("{}.log", get_logfile_stem()));
			let (non_blocking, guard) = tracing_appender::non_blocking(file_writer);
			let layer = layer().with_writer(non_blocking.with_max_level(loglevel)).boxed();

			Some((layer, guard))
		}
		_ => None,
	}
}

/// Returns a log filename stem (a filename without an extension).
#[inline]
fn get_logfile_stem() -> String {
	let default_name: String = String::from("p4-analyzer");
	let executable_name = current_exe()
		.ok()
		.and_then(|path_buffer| path_buffer.file_stem().map(|s| s.to_os_string()).and_then(|s| s.into_string().ok()));

	executable_name.unwrap_or(default_name)
}

/// Adapts a [`Command`] and makes it runnable.
///
/// Since [`Command`] instances are runnable with a [`CancellationToken`], a [`RunnableCommand`] will cancel its underlying
/// command when receiving a 'Ctrl-C' signal.
struct RunnableCommand<C: Command>(C);

impl<C: Command> RunnableCommand<C> {
	/// Executes the adapted command.
	///
	/// The supplied command will be invoked with a [`CancellationToken`] that is canceled upon receiving a 'Ctrl-C' signal (if
	/// it is supported by the platform).
	async fn run(&self) {
		let Self(cmd) = self;

		let count = Arc::new(AtomicU8::new(0));

		let cancellation_source = CancellationTokenSource::new();
		let cancellation_token = cancellation_source.token().clone();

		ctrlc::set_handler(move || {
			let prev_count = count.fetch_add(1, Ordering::Relaxed);

			if prev_count == 0 {
				eprintln!();
				eprintln!("(To forcibly exit, press 'Ctrl+C' again)");

				cancellation_source.cancel();
			}

			if prev_count > 0 {
				process::exit(-1);
			}
		})
		.expect("'Ctrl-C' handling is not available for this platform.");

		match cmd.run(cancellation_token).await {
			Ok(_) => {}
			Err(err) => match err {
				CommandInvocationError::Cancelled => println!("{}", err),
				_ => eprintln!("{}", err),
			},
		};
	}

	/// Retrieves any additional logging layers that have been configured by the underlying command.
	fn logging_layers(&self) -> Vec<Box<dyn Layer<Registry> + Send + Sync + 'static>> {
		let Self(cmd) = self;

		cmd.logging_layers::<Registry>()
	}
}

// Unit test fixtures.
#[cfg(test)]
mod tests;
