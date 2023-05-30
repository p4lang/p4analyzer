// Web Assembly is a sandbox, and by design shouldn't have access to network, file systenm, or underlying operating system.
// Simply removing this file system code from wasm build is the best way
#[cfg(not(target_arch = "wasm32"))] // should be sufficient
#[cfg(not(target_family = "wasm"))] // extra safety
pub mod native_fs {
    use std::sync::Arc;

    use analyzer_abstractions::{fs::EnumerableFileSystem, lsp_types::{Url, TextDocumentIdentifier}, BoxFuture};
    use async_channel::Sender;
    use cancellation::CancellationToken;
    use notify::RecommendedWatcher;
    use regex::Regex;

    use crate::json_rpc::message::Message;

    struct NativeFs {
        token: Arc<CancellationToken>,      // needed to tell the thread to stop
        request_sender: Sender<Message>,    // needed to let the analyzer-host that the file has been changed
    }
    
    impl NativeFs {
        pub fn new(token: Arc<CancellationToken>, request_sender: Sender<Message>) -> Self {
            NativeFs { token, request_sender }
        }

        fn start_file_watch() {
            todo!()
        }
    }
    
    // EnumerableFileSystem part of NativeFs will just use std::fs methods for the functions
    impl EnumerableFileSystem for NativeFs {
        fn enumerate_folder<'a>(
		        &'a self,
		        folder_uri: Url,
		        file_pattern: String,
	        ) -> BoxFuture<'a, Vec<TextDocumentIdentifier>> {
                
                async fn enumerate_folder(
                    folder_uri: Url,
                    file_pattern: String,
                ) -> Vec<TextDocumentIdentifier> {
                    let folder = folder_uri.path();

                    let res = std::fs::read_dir(folder);    // make async somehow
                    if res.is_err() { return Vec::new(); }
                    let dir_itr = res.unwrap();

                    let re = Regex::new(file_pattern.as_str()).unwrap();
                    let mut output = Vec::new();
                    
                    for file in dir_itr {   // make async somehow
                        if re.is_match(file.as_ref().unwrap().file_name().to_str().unwrap()) {
                            let path = file.unwrap().path();
                            output.push(TextDocumentIdentifier { uri: Url::parse(path.to_str().unwrap()).unwrap()})
                        }
                    }

                    output
                }
        
                Box::pin(enumerate_folder(folder_uri, file_pattern))
        }

        fn file_contents<'a>(&'a self, file_uri: Url) -> BoxFuture<'a, Option<String>> {
            async fn file_contents(file_uri: Url) -> Option<String> {
                let path = file_uri.path();

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