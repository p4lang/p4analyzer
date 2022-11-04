use analyzer_abstractions::{Logger, lsp_types::request};
use analyzer_host::{
	AnalyzerHost, MessageChannel,
	json_rpc::message::*
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern {
	#[wasm_bindgen(js_namespace = console)]
	fn log(s: &str);
}

#[wasm_bindgen]
pub fn run() {
	spawn_local(async move {
		try_run().await;

		log("The host has completed.");
	});
}

async fn try_run() {
	let cts = cancellation::CancellationTokenSource::new();
	let request_channel: MessageChannel = async_channel::unbounded::<Message>();
	let response_channel: MessageChannel = async_channel::unbounded::<Message>();
	// let host = AnalyzerHost::new(request_channel, response_channel, &ConsoleLogger { });

	// TODO: Add handler for Ctrl-C that calls `cts.cancel()`
	// cts.cancel_after(std::time::Duration::from_millis(500));
	// cts.cancel();

	// match host.start(&cts).await {
	// 	Ok(_) => { }
	// 	Err(err) => log(&format!("{}", err))
	// }
}

struct ConsoleLogger { }

impl Logger for ConsoleLogger {
    fn log_message(&self, msg: &str) {
			log(msg);
    }

    fn log_error(&self, msg: &str) {
      log(msg);
    }
}
