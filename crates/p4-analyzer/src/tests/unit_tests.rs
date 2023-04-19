mod main_tests {
	use crate::{
		cli::flags::{self, P4Analyzer, P4AnalyzerCmd},
		create_default_logging_layer, get_logfile_stem,
	};
	use analyzer_host::tracing::tracing_subscriber::Registry;

	#[test]
	fn test_create_default_logging_layer() {
		let cmd = P4Analyzer {
			logpath: None,
			loglevel: None,
			tcp: None,
			subcommand: P4AnalyzerCmd::Server(flags::Server { stdio: false }),
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

mod driver_tests {
	use std::{net::{TcpStream, SocketAddr}, sync::{Arc, Mutex, RwLock}, io::{stdout, stdin, BufWriter,BufReader, self, Read, Write}};

use crate::driver::{console_driver, tcp_driver, buffer_driver, BufferStruct};
use analyzer_host::{json_rpc::message::{Message, Request, Notification, Response}, MessageChannel};
use cancellation::CancellationTokenSource;
extern crate queues;
use queues::*;
use ::tester::tester::tester::default_initialize_message;

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

	async fn buffer_test(buffer: &mut BufferStruct, channels: MessageChannel) {
		buffer.allow_read();	// Mimic Driver sending initialize message to Anaylzer Host
		let mess = channels.1.recv().await.unwrap();	// Mimic reading Anaylzer Host buffer
		assert_eq!(mess.to_string(), "initialize:0");

		let message = Message::Response(Response{
    		id: 0.into(),
    		result: None,
    		error: None,
		});
		channels.0.send(message).await.unwrap();	// Mimic Anaylzer Host sending message to Driver
		let (mess, count) = buffer.get_output_buffer_blocking();	// Mimic reading driver buffer
		assert_eq!(mess, "Content-Length: 24\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":0}");
		assert_eq!(count, 1);

		// Anaylzer host is responsible for closing the channels that lets the driver know to shutdown
		channels.0.close();
		channels.1.close();
	}

	#[tokio::test]
	async fn test_buffer_driver() {
		let mut queue: Queue<Message> = queue![];

		let message = default_initialize_message();
		queue.add(message).unwrap();		
		
		let mut buffer = BufferStruct::new(queue);
		let driver = buffer_driver(buffer.clone());

		let token = CancellationTokenSource::new();
		let future = driver.start(token.token().clone());
		let test_future = buffer_test(&mut buffer, driver.get_message_channel());
		let _ = tokio::join!(future, test_future);
	}

	#[tokio::test]
	async fn test_http_driver() {
		/*let socket_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
		let stream = TcpStream::connect(socket_addr.clone()).unwrap();
		let driver = http_driver(socket_addr.clone());*/
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
