// Web Assembly is a sandbox, and by design shouldn't have access to network, file systenm, or underlying operating system.
// Simply removing this file system code from wasm build is the best way
#[cfg(not(target_arch = "wasm32"))]
pub mod native_fs {
    use std::sync::Arc;

    use analyzer_abstractions::fs::EnumerableFileSystem;
    use async_channel::Sender;
    use cancellation::CancellationToken;

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
		        folder_uri: analyzer_abstractions::lsp_types::Url,
		        file_pattern: String,
	        ) -> analyzer_abstractions::BoxFuture<'a, Vec<analyzer_abstractions::lsp_types::TextDocumentIdentifier>> {
            todo!()
        }

        fn file_contents<'a>(&'a self, file_uri: analyzer_abstractions::lsp_types::Url) -> analyzer_abstractions::BoxFuture<'a, Option<String>> {
            todo!()
        }
    }
}