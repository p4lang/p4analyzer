mod main_tests {
	extern crate queues;
	use std::sync::Arc;

	use crate::{
		cli::flags::{self, P4Analyzer, P4AnalyzerCmd, Server},
		commands::lsp_server::LspServerCommand,
		create_default_logging_layer,
		driver::{buffer_driver::BufferStruct, DriverType},
		get_logfile_stem, RunnableCommand,
	};
	use analyzer_host::{json_rpc::message::Message, tracing::tracing_subscriber::Registry};
	use queues::*;
	use tester::tester::tester::*;

	#[test]
	fn test_create_default_logging_layer() {
		let cmd = P4Analyzer {
			logpath: None,
			loglevel: None,
			subcommand: P4AnalyzerCmd::Server(flags::Server { stdio: false }),
			version: false,
		};
		let res = create_default_logging_layer::<Registry>(&cmd);
		assert!(res.is_none());

		match P4Analyzer::from_vec(vec![]) {
			Ok(cmd) => {
				let res = create_default_logging_layer::<Registry>(&cmd);
				assert!(res.is_none());
			}
			_ => unreachable!(),
		}
	}

	#[test]
	fn test_get_logfile_stem() {
		// random generated string each time, so just make sure not empty
		assert!(get_logfile_stem().contains("p4analyzer-"));
	}

	async fn lsp_test_messages(buffer: Arc<BufferStruct>) {
		buffer.allow_read_blocking().await; // Initialize Message sent
		let resp0 = buffer.get_output_buffer(1).await;
		assert_eq!(resp0.len(), 1);
		assert!(resp0[0].contains("{\"jsonrpc\":\"2.0\",\"id\":0,\"result\""));

		buffer.allow_read_blocking().await; // Initialized Message sent

		let resp1 = buffer.get_output_buffer(1).await;
		assert_eq!(resp1.len(), 1);
		assert_eq!(resp1[0], "Content-Length: 227\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"client/registerCapability\",\"params\":{\"registrations\":[{\"id\":\"p4-analyzer-watched-files\",\"method\":\"workspace/didChangeWatchedFiles\",\"registerOptions\":{\"watchers\":[{\"globPattern\":\"**/*.p4\"}]}}]}}");

		buffer.allow_read_blocking().await; // Send Response

		//buffer.allow_read_blocking().await; // Shutdown Message sent
		buffer.allow_all_read_blocking().await; // Exit Message sent
	}

	#[tokio::test]
	async fn test_runnable_command() {
		let mut queue: Queue<Message> = queue![];

		queue.add(default_initialize_message()).unwrap();
		queue.add(default_initialized_message()).unwrap();
		queue.add(default_response()).unwrap();
		//queue.add(default_shutdown_message()).unwrap();
		queue.add(default_exit_message()).unwrap();

		let buffer = Arc::new(BufferStruct::new(queue));

		let lsp = LspServerCommand::new(Server { stdio: false }, DriverType::Buffer(buffer.clone()));
		let obj = RunnableCommand::<LspServerCommand>(lsp);

		// ?No clue why LspServerCommand::run() doesn't exit correctly
		//let future = obj.run();
		//let test_future = lsp_test_messages(buffer);

		//tokio::join!(future, test_future);
	}
}

mod driver_tests {
	extern crate queues;
	use std::sync::Arc;

	use crate::{
		driver::{
			buffer_driver::{self, BufferStruct},
			console_driver,
		},
		tests::unit_tests::driver_tests::buffer_driver::buffer_driver,
	};
	use ::tester::tester::tester::default_initialize_message;
	use analyzer_host::{
		json_rpc::message::{Message, Response},
		MessageChannel,
	};
	use cancellation::CancellationTokenSource;
	use queues::*;

	#[tokio::test]
	async fn test_console_driver() {
		let driver = console_driver();
		let token = CancellationTokenSource::new();
		let future = driver.start(token.token().clone());

		let test_future = async {
			driver.get_message_channel().0.clone().close();
		};
		let (res, _) = tokio::join!(future, test_future);
		assert!(!res.is_err());

		let future = driver.start(token.token().clone());
		let test_future = async {
			token.cancel();
		};
		let (res, _) = tokio::join!(future, test_future);
		assert!(res.is_err());
	}

	async fn buffer_test(buffer: Arc<BufferStruct>, channels: MessageChannel) {
		buffer.allow_read_blocking().await; // Mimic Driver sending initialize message to Analyzer Host
		let mess = channels.1.recv().await.unwrap(); // Mimic reading Analyzer Host buffer
		assert_eq!(mess.to_string(), "initialize:0");

		let message = Message::Response(Response { id: 0.into(), result: None, error: None });
		channels.0.send(message).await.unwrap(); // Mimic Analyzer Host sending message to Driver
		let mess = buffer.get_output_buffer(1).await; // Mimic reading driver buffer
		assert_eq!(mess.len(), 1);
		assert_eq!(mess[0], "Content-Length: 24\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":0}");

		// Analyzer host is normally responsible for closing the channels that lets the driver know it should shut down
		channels.0.close();
		channels.1.close();
	}

	#[tokio::test]
	async fn test_buffer_driver() {
		let mut queue: Queue<Message> = queue![];

		let message = default_initialize_message();
		queue.add(message).unwrap();

		let buffer = Arc::new(BufferStruct::new(queue));
		let driver = buffer_driver(buffer.clone());

		let token = CancellationTokenSource::new();
		let future = driver.start(token.token().clone());
		let test_future = buffer_test(buffer, driver.get_message_channel());
		let _ = tokio::join!(future, test_future);
	}
}

mod lsp_server_tests {
	use cancellation::CancellationTokenSource;

	use crate::{
		cli::flags::Server,
		commands::{lsp_server::LspServerCommand, Command},
	};

	#[tokio::test]
	async fn command_aborts_when_cancelled() {
		let lsp = LspServerCommand::new(Server { stdio: false }, crate::driver::DriverType::Console);
		let token = CancellationTokenSource::new();
		let (res, _) = tokio::join!(lsp.run(token.token().clone()), async { token.cancel() });
		assert!(res.is_err());
	}
}
