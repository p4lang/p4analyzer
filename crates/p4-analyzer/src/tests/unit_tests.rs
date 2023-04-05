mod main_tests {

	use crate::{
		cli::flags::{self, P4Analyzer, P4AnalyzerCmd},
		create_default_logging_layer, get_logfile_stem
	};
	use analyzer_host::tracing::tracing_subscriber::Registry;
	use serial_test::parallel;

	#[test]
	#[parallel]

	fn test_create_default_logging_layer() {

		let cmd = P4Analyzer {
			logpath: None,
			loglevel: None,
			subcommand: P4AnalyzerCmd::Server(flags::Server { stdio: false })
		};

		let res = create_default_logging_layer::<Registry>(&cmd);

		assert!(res.is_none());

		match P4Analyzer::from_vec(vec![]) {
			Ok(cmd) => {

				let res = create_default_logging_layer::<Registry>(&cmd);

				assert!(res.is_none());
			}
			_ => unreachable!()
		}
	}

	#[test]
	#[parallel]

	fn test_get_logfile_stem() {

		// random generated string each time, so just make sure not empty
		assert!(get_logfile_stem().contains("p4analyzer-"));
	}

	/*
	async fn lsp_test_messages(sender : &ConsoleDriver ) {
		let initialize_params = analyzer_abstractions::lsp_types::InitializeParams{ ..Default::default() };
		let json = serde_json::json!(initialize_params);

		let message = Message::Request(Request{
			id: 0.into(),
			method: String::from("initialize"),
			params: json,
		});
		(*sender).send_message_in(message.clone()).await.unwrap();

		let initialize_params = analyzer_abstractions::lsp_types::InitializedParams{};
		let json = serde_json::json!(initialize_params);
		let message = Message::Notification(Notification{
			method: String::from("initialized"),
			params: json,
		});
		(*sender).send_message_in(message.clone()).await.unwrap();

		let message = Message::Notification(Notification{
			method: String::from("exit"),
			params: Value::Null,
		});
		(*sender).send_message_in(message.clone()).await.unwrap();
	}

	#[tokio::test]
	#[serial]
	async fn test_runnable_command() {
		let lsp = LspServerCommand::new(Server{stdio:false});
		let obj = RunnableCommand::<LspServerCommand>(lsp);
		let sender: &ConsoleDriver  = &obj.0.console_driver;
		let future = RunnableCommand::<LspServerCommand>::run(&obj);
		let future2 = lsp_test_messages(sender);

		let mut reader = tester::tester::tester::start_stdout_capture();

		tokio::join!(future, future2);

		let result = tester::tester::tester::get_stdout_capture(&mut reader, 1).await;
		// The message is very long so only match start of it
		assert!(result.unwrap().contains("Content-Length: 423\r\n\r\n{\"jsonrpc\":\"2.0\","));
	}
	*/
}

mod stdio_tests {

	use crate::stdio::ConsoleDriver;
	use cancellation::CancellationTokenSource;
	use serial_test::parallel;

	#[tokio::test]
	#[parallel]

	async fn test_console_driver() {

		let driver = ConsoleDriver::new();

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
	/*
	#[tokio::test]
	#[serial]
	async fn test_receive_task() {
		let obj = ConsoleDriver::new();
		let (sender, receiver) = obj.stdout_channel.clone();
		let receiver_task = std::thread::spawn(move || ConsoleDriver::receiver_task(receiver));

		let mess = Message::Notification(Notification{
			method: "initialized".into(),
			params: Value::Null,
		});

		let mut capture = start_stdout_capture();

		sender.send(mess).await.unwrap();

		let output = get_stdout_capture(&mut capture, 1).await;
		assert_eq!(output.unwrap(), "Content-Length: 40\r\n\r\n{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}");

		stop_stdout_capture(capture);
		sender.close();
		receiver_task.join().unwrap();
	}
	*/
}

mod lsp_server_tests {

	use cancellation::CancellationTokenSource;

	use crate::{
		cli::flags::Server,
		commands::{lsp_server::LspServerCommand, Command}
	};

	#[tokio::test]

	async fn command_aborts_when_cancelled() {

		let lsp = LspServerCommand::new(Server { stdio: false });

		let token = CancellationTokenSource::new();

		let (res, _) = tokio::join!(lsp.run(token.token().clone()), async { token.cancel() });

		assert!(res.is_err());
	}
}
