// A class for loading, running and testing premade or custom P4 files
#[cfg(debug_assertions)]
pub mod tester {
	use gag::BufferRedirect;
	use std::{
		fs,
		io::{stdout, Read, Write},
		path::PathBuf,
		sync::{RwLock, RwLockWriteGuard},
		time::Duration,
	};

	use lazy_static::lazy_static;
	lazy_static! {
		// Is lazy_static because it's a runtime generated value
		static ref PATH : String = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/").into_os_string().into_string().unwrap();

		// Using RwLock as stdio(console_driver) runs on a different thread to LSP and tests
		// It's a global variable as only 1 needs to exist for the whole program
		//static ref MESSAGE_HANDLER : RwLock<usize> = RwLock::new(0);
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
	/*
	// A crude method for making an event handler for when a message is sent
	// returns a write lock so that it remained locked until the buffer of stdout is written to in parent method
	pub fn message_sent() -> RwLockWriteGuard<'static, usize> {
		let mut lock = MESSAGE_HANDLER.write().unwrap();
		*lock += 1;
		lock
	}

	// A crude method for making an event listener for when a message is sent
	// milli: how long it waits until it times out (not very accurate)
	// num_mess: how many messages to wait for
	// returns write lock because parent method will reset the count once it's read the buffer (this method only reads the count)
	async fn await_message(milli : usize, num_mess : usize) -> Option<RwLockWriteGuard<'static, usize>> {
		let mut count : usize = 0;  // timeout counter
		loop {
			let lock = MESSAGE_HANDLER.write().unwrap();    // hold write lock for resetting counter
			if *lock >= num_mess {  // only read value
				return Some(lock);
			}
			drop(lock);  // release lock
			async_std::task::sleep(Duration::from_millis(1)).await; // crude message handler
			count += 10;
			if milli <= count {  // timeout solution
				return None;
			}
		}
	}

	// Using the gag cargo container to redirect buffer
	// Only works in tests if --nocapture is set
	// `cargo t` is a custom cargo test that is made to work with this
	pub fn start_stdout_capture() -> BufferRedirect {
		stdout().flush().unwrap();  // Increase the chances that stdout is empty (although this is not guaranteed with parallel testing)
		let mut lock = MESSAGE_HANDLER.write().unwrap();
		*lock = 0;
		BufferRedirect::stdout().unwrap()
	}

	// pass the buffer redirect by reference
	// specify how many messages you're waiting for
	// returns Option as it will be None if waiting for message times out
	pub async fn get_stdout_capture(reader : &mut BufferRedirect, num_messages : usize) -> Option<String> {
		if num_messages == 0 {
			return None;
		}
		let mut output = String::new();

		let lock = await_message(3000, num_messages).await; // hold a write lock for later
		if lock.is_none() {
			return None;  // It timed out
		}

		stdout().flush().unwrap();
		let _lock = stdout().lock();    // Don't allow anyone to write while we read from it

		(*reader).read_to_string(&mut output).unwrap(); // This reads stdout

		*lock.unwrap() = 0; // reset message count
		Some(output)
	}

	// takes ownership of buffer
	pub fn stop_stdout_capture(reader: BufferRedirect) {
		drop(reader);
	}
	*/
}

#[cfg(test)]
mod tests {
	use super::tester;
	use serial_test::serial; // This crate doesn't seem to work :(

	#[test]
	#[serial]
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
	/*
	fn simulate_message(string : &str) {
		let _lock = tester::message_sent();
		println!("{}", string);
	}

	#[tokio::test]
	#[serial]
	async fn test_stdout_capture() {
		simulate_message("readable!");
		let mut obj = tester::start_stdout_capture();
		//let mut obj = tester::start_stdout_capture(); // <-- causes runtime error

		simulate_message("not readable");
		let str = tester::get_stdout_capture(&mut obj, 1).await;
		assert_eq!(str.unwrap(), "not readable\n");

		let str = tester::get_stdout_capture(&mut obj, 0).await;
		assert!(str.is_none());

		simulate_message("hello");
		simulate_message("world");
		let str = tester::get_stdout_capture(&mut obj, 2).await;
		assert_eq!(str.unwrap(), "hello\nworld\n");

		tester::stop_stdout_capture(obj);
		//obj;  // <-- this causes a compilier error
		simulate_message("readable again!");

		let mut obj = tester::start_stdout_capture();

		simulate_message("not readable2");
		let str = tester::get_stdout_capture(&mut obj, 1).await;
		assert_eq!(str.unwrap(), "not readable2\n");

		tester::stop_stdout_capture(obj);
		//obj;  // <-- this causes a compilier error
		simulate_message("readable again2!");
	}
	*/
}
