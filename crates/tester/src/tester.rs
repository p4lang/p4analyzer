// A class for loading, running and testing premade or custom P4 files
pub mod tester {
	use std::{
		fs,
		path::PathBuf,
	};
	use analyzer_host::json_rpc::message::{Message, Request, Notification};
	use lazy_static::lazy_static;
	use serde_json::Value;

	lazy_static! {
		// Is lazy_static because it's a runtime generated value
		static ref PATH : String = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/").into_os_string().into_string().unwrap();
	}

	// List of p4 files in the example folder for checking & opening the file
	static FILES: [&str; 1] = ["example0.p4"];

	// uses the FILES[] index to load files without specifying the file name
	pub fn load_file_num(file_number: usize) -> Option<String> {
		if file_number >= FILES.len() {
			return None;
		}
		load_file_str(FILES[file_number])
	}

	// Uses the name of the file instead but will check the file name is contained in the array
	pub fn load_file_str(file_name: &str) -> Option<String> {
		if !FILES.contains(&file_name) {
			println!("File name not recognized");
			return None;
		}

		let full_path = format!("{}{}", *PATH, file_name);
		println!("Loading {}...", full_path);

		match fs::read(full_path.clone()) {
			Ok(content) => Some(String::from_utf8(content).unwrap()),
			Err(_) => {
				println!("Failed to open {}!!!", full_path);
				None
			}
		}
	}

	pub fn default_initialize_message() -> Message {
		let initialize_params = analyzer_abstractions::lsp_types::InitializeParams{ ..Default::default() };
		let json = serde_json::json!(initialize_params);
		Message::Request(Request{
			id: 0.into(),
			method: String::from("initialize"),
			params: json,
		})
	}

	pub fn default_initialized_message() -> Message {
		let initialized_params = analyzer_abstractions::lsp_types::InitializedParams{};
		let json = serde_json::json!(initialized_params);
		Message::Notification(Notification{
			method: String::from("initialized"),
			params: json,
		})
	}

	pub fn default_shutdown_message() -> Message {
		Message::Request(Request{
			id: 0.into(),
			method: String::from("shutdown"),
			params: Value::Null,
		})
	}

	pub fn default_exit_message() -> Message {
		Message::Notification(Notification{
			method: String::from("exit"),
			params: Value::Null,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::tester;

	#[test]
	fn test_load() {
		// invalid entreis
		assert_eq!(tester::load_file_num(std::usize::MAX), None);
		assert_eq!(tester::load_file_str("doesnt_exist"), None);
		// check for a return
		let content = tester::load_file_num(0);
		assert!(content.is_some());
		assert!(tester::load_file_str("example0.p4").is_some());
		// check content
		#[cfg(target_os = "windows")]
		{	assert_eq!(content.unwrap(), "#include <core.p4>\r\n");	}	// Testing both CRLF and LF as Wasm & LSP is platform specific in EOL
		#[cfg(not(target_os = "windows"))]
		{	assert_eq!(content.unwrap(), "#include <core.p4>\n");	}
	}
}
