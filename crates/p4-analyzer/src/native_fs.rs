pub mod native_fs {
	use std::{any::Any, path::{PathBuf, Path}, sync::Arc, thread};

	use analyzer_abstractions::{
		fs::{AnyEnumerableFileSystem, EnumerableFileSystem},
		lsp_types::{FileChangeType, FileEvent, TextDocumentIdentifier, Url},
		BoxFuture,
	};
	use async_channel::Sender;
	use cancellation::CancellationToken;
	use futures::lock::Mutex;
	use notify::{Event, RecursiveMode, Watcher};
	use glob::glob;
	use analyzer_host::json_rpc::message::{Message, Notification};

	pub struct NativeFs {
		//	watcher: notify::RecommendedWatcher,
		//	watching: Vec<Url>,
	}

	impl NativeFs {
		pub fn new() -> AnyEnumerableFileSystem {
			/*let watcher = notify::recommended_watcher(move |res| match res {
				Ok(event) => {
					if let Some(mess) = Self::file_change(event) {
						let _ = futures::executor::block_on(request_sender.send(mess));
					}
				}
				Err(e) => println!("watch error: {:?}", e),
			})
			.unwrap();*/

			//let object: Arc<Mutex<Box<dyn EnumerableFileSystem + Send + Sync + 'static>>> = Arc::new(Mutex::new(Box::new(NativeFs { watcher, watching: Vec::new() })));
			let object: AnyEnumerableFileSystem = Box::new(NativeFs {});

			/*
			// Start a new thread as CancellationToken::run() is blocking
			// only 1 FileSystem should exist per P4Analyzer so not to worried about many child threads
			let clone = object.clone();
			thread::spawn(move || {
				token.run(|| {
					futures::executor::block_on(clone.lock()).as_any().downcast_mut::<NativeFs>().unwrap().stop_watching_all();
				},
				|| {});
			});
			*/
			object
		}
		/*
		// No current way to called the `watcher.unwatch()` function
		fn start_folder_watch(&mut self, folder_uri: &Url) {
			self.watching.push(folder_uri.clone()); // add path to vector
			self.watcher.watch(folder_uri.path().as_ref(), RecursiveMode::Recursive).unwrap(); // start watcher
		}

		fn stop_watching_all(&mut self) {
			for elm in &self.watching {
				self.watcher.unwatch(elm.path().as_ref()).unwrap();
			}
			self.watching.clear();
		}

		// has to be manually called
		pub fn stop_folder_watch(&mut self, folder_uri: &Url) {
			self.watching.retain(|x| *x != *folder_uri); // remove from vector
			self.watcher.unwatch(folder_uri.path().as_ref()).unwrap();	// if exists with unwatch it
		}

		fn file_change(event: Event) -> Option<Message> {
			let mut paths = event.paths;

			paths.retain(|x| x.ends_with(".p4"));
			if paths.is_empty() {
				return None;
			}

			match event.kind {
				notify::EventKind::Any => None,
				notify::EventKind::Access(_) => None,
				notify::EventKind::Create(_) => Self::create_message(paths, FileChangeType::CREATED),
				notify::EventKind::Modify(_) => Self::create_message(paths, FileChangeType::CHANGED),
				notify::EventKind::Remove(_) => Self::create_message(paths, FileChangeType::DELETED),
				notify::EventKind::Other => None,
			}
		}

		fn create_message(paths: Vec<PathBuf>, event_type: FileChangeType) -> Option<Message> {
			let files = paths
				.into_iter()
				.map(|x| FileEvent { uri: Url::parse(x.to_str().unwrap()).unwrap(), typ: event_type })
				.collect();

			let create_files_params = analyzer_abstractions::lsp_types::DidChangeWatchedFilesParams { changes: files };
			let params = serde_json::json!(create_files_params);

			// no sure of the difference between `workspace/didChangeWatchedFiles` and `workspace/didDeleteFiles` or `workspace/didCreateFiles`
			Some(Message::Notification(Notification { method: "workspace/didChangeWatchedFiles".into(), params }))
		}
		*/
	}

	// EnumerableFileSystem part of NativeFs will just use std::fs methods for the functions
	impl EnumerableFileSystem for NativeFs {
		fn as_any(&mut self) -> &mut dyn Any { self }

		fn is_native(&self) -> bool { true }

		fn enumerate_folder<'a>(
			&'a self,
			folder_uri: Url,
			file_pattern: String,
		) -> BoxFuture<'a, Vec<TextDocumentIdentifier>> {
			//self.start_folder_watch(&folder_uri); // add folder to watch list

			async fn enumerate_folder(folder_uri: Url, file_pattern: String) -> Vec<TextDocumentIdentifier> {
				let folder = folder_uri.to_file_path().unwrap();
				let glob_pattern = folder.to_str().unwrap().to_owned() + file_pattern.as_str();

				let mut output = Vec::new();

				for files in glob(glob_pattern.as_str()).expect("Failed to read glob pattern") {
					match files {
						Ok(path) => output.push(TextDocumentIdentifier { uri: Url::from_file_path(path).unwrap() }),
						Err(e) => println!("{:?}", e),
					}
				}

				output
			}

			Box::pin(enumerate_folder(folder_uri, file_pattern))
		}

		fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>> {
			async fn file_contents(file_uri: Url) -> Option<String> {
				let path = file_uri.to_file_path().unwrap();

				let data = tokio::fs::read(path).await;

				match data {
					Ok(data) => Some(String::from_utf8(data).unwrap()),
					Err(_) => None,
				}
			}

			Box::pin(file_contents(file_uri))
		}
	}
}

#[cfg(test)]
mod tests {
	use analyzer_abstractions::lsp_types::Url;
	use super::{native_fs::NativeFs};
	use std::{fs, path::PathBuf};
	use analyzer_host::lsp::RELATIVE_P4_SOURCEFILES_GLOBPATTERN;

	#[tokio::test]
	async fn test_enumerate_folder() {
		// build test file_name location
		let dir_name = "./test_directory0";
		fs::create_dir(dir_name).expect("Failed to create directory");
		fs::File::create("./test_directory0/file1.p4").unwrap();
		fs::File::create("./test_directory0/file2.not_p4").unwrap();
		fs::File::create("./test_directory0/file3.p4").unwrap();
		fs::File::create("./test_directory0/file4.p4").unwrap();

		// build NativeFs
		let object = NativeFs::new();
		let url = Url::from_directory_path(fs::canonicalize(&PathBuf::from(dir_name)).unwrap()).unwrap();

		// tests
		let result = object.enumerate_folder(url.clone(), RELATIVE_P4_SOURCEFILES_GLOBPATTERN.into()).await;		
		
		// Number of results
		assert_eq!(result.len(), 3);
		// URL content
		assert_eq!(result[0].uri.scheme(), "file");
		assert_eq!(result[0].uri.to_file_path().unwrap().file_name().unwrap(), "file1.p4");
		assert_eq!(result[1].uri.scheme(), "file");
		assert_eq!(result[1].uri.to_file_path().unwrap().file_name().unwrap(), "file3.p4");
		assert_eq!(result[2].uri.scheme(), "file");
		assert_eq!(result[2].uri.to_file_path().unwrap().file_name().unwrap(), "file4.p4");

		// test recursive files & the OS file_name system has changes, so enumerate_folder() should give different result
		fs::create_dir("./test_directory0/subdirectory0").expect("Failed to create directory");
		fs::File::create("./test_directory0/subdirectory0/file5.p4").unwrap();
		fs::File::create("./test_directory0/subdirectory0/file6.not_p4").unwrap();
		fs::File::create("./test_directory0/subdirectory0/file7.p4").unwrap();
		let result = object.enumerate_folder(url.clone(), RELATIVE_P4_SOURCEFILES_GLOBPATTERN.into()).await;

		// Number of results
		assert_eq!(result.len(), 5);
		// URL content
		assert_eq!(result[3].uri.scheme(), "file");
		assert_eq!(result[3].uri.to_file_path().unwrap().file_name().unwrap(), "file5.p4");
		assert!(result[3].uri.to_file_path().unwrap().parent().unwrap().to_str().unwrap().ends_with("subdirectory0"));
		assert_eq!(result[4].uri.scheme(), "file");
		assert_eq!(result[4].uri.to_file_path().unwrap().file_name().unwrap(), "file7.p4");
		assert!(result[4].uri.to_file_path().unwrap().parent().unwrap().to_str().unwrap().ends_with("subdirectory0"));

		// clean up
		fs::remove_dir_all(dir_name).expect("Failed to delete directory");
	}

	#[tokio::test]
	async fn test_file_content() {
		// build test file_name
		let dir_name = "./test_directory1";
		let file_name = "./test_directory1/file0.p4";
		let test_string = "hello world\n";
		fs::create_dir(dir_name).expect("Failed to create directory");
		fs::File::create(file_name).unwrap();
		fs::write(file_name, test_string).unwrap();

		// build NativeFs
		let object = NativeFs::new();
		let url = Url::from_file_path(fs::canonicalize(&PathBuf::from(file_name)).unwrap()).unwrap();

		// test
		let res = object.file_contents(url.clone()).await;

		assert!(res.is_some());
		assert_eq!(res.unwrap(), test_string);

		// change file_name content
		let test_string = "\n No longer hello world but instead this\n hmmm\n";
		fs::write(file_name, test_string).unwrap();
		let res = object.file_contents(url.clone()).await;

		assert!(res.is_some());
		assert_eq!(res.unwrap(), test_string);

		// clean up
		fs::remove_dir_all(dir_name).expect("Failed to delete directory");
	}

	#[tokio::test]
	async fn test_both_together() {
		// build test file_name
		let dir_name = "./test_directory2";
		let file_name = "./test_directory2/file0.p4";
		let test_string = "This is file0.p4 content\n";
		fs::create_dir(dir_name).expect("Failed to create directory");
		fs::File::create(file_name).unwrap();
		fs::write(file_name, test_string).unwrap();

		// build NativeFs
		let object = NativeFs::new();
		let url = Url::from_directory_path(fs::canonicalize(&PathBuf::from(dir_name)).unwrap()).unwrap();

		// get URL from enumerate_folder()
		let result = object.enumerate_folder(url.clone(), RELATIVE_P4_SOURCEFILES_GLOBPATTERN.into()).await;
		// pass URL into file_content()
		let conent = object.file_contents(result[0].uri.clone()).await;
		
		// tests
		assert!(conent.is_some());
		assert_eq!(conent.unwrap(), test_string);

		// clean up
		fs::remove_dir_all(dir_name).expect("Failed to delete directory");
	}
}
