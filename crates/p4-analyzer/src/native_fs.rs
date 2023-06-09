pub mod native_fs {
	use analyzer_abstractions::{
		fs::{AnyEnumerableFileSystem, EnumerableFileSystem},
		lsp_types::{TextDocumentIdentifier, Url},
		BoxFuture,
	};
	use glob::glob;

	pub struct NativeFs {}

	impl NativeFs {
		pub fn new() -> AnyEnumerableFileSystem { Box::new(NativeFs {}) }
	}

	// EnumerableFileSystem part of NativeFs will just use std::fs methods for the functions
	impl EnumerableFileSystem for NativeFs {
		fn enumerate_folder<'a>(
			&'a self,
			folder_uri: Url,
			file_pattern: String,
		) -> BoxFuture<'a, Vec<TextDocumentIdentifier>> {
			async fn enumerate_folder(folder_uri: Url, file_pattern: String) -> Vec<TextDocumentIdentifier> {
				let mut folder_uri = folder_uri.clone();
				folder_uri.path_segments_mut().unwrap().push(&file_pattern);

				let glob_pattern = folder_uri.to_file_path().unwrap();
				let glob_pattern = glob_pattern.to_str().unwrap();

				let mut output = Vec::new();

				for files in glob(glob_pattern).expect("Failed to read glob pattern") {
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
	use super::native_fs::NativeFs;
	use analyzer_abstractions::lsp_types::Url;
	use analyzer_host::lsp::RELATIVE_P4_SOURCEFILES_GLOBPATTERN;
	use std::{fs, path::PathBuf};

	struct CleanUp {
		dir: String,
	}

	impl Drop for CleanUp {
		fn drop(&mut self) { fs::remove_dir_all(self.dir.clone()).expect("Failed to delete directory"); }
	}

	#[tokio::test]
	async fn test_enumerate_folder() {
		// build test file_name location
		let dir_name = "./test_directory0";
		let clean_obj = CleanUp { dir: dir_name.into() };

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
		drop(clean_obj);
	}

	#[tokio::test]
	async fn test_file_content() {
		// build test file_name
		let dir_name = "./test_directory1";
		let clean_obj = CleanUp { dir: dir_name.into() };

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
		drop(clean_obj);
	}

	#[tokio::test]
	async fn test_both_together() {
		// build test file_name
		let dir_name = "./test_directory2";
		let clean_obj = CleanUp { dir: dir_name.into() };

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
		drop(clean_obj);
	}
}
