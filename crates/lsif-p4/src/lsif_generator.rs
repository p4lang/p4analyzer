use std::{sync::Arc, rc::Rc, fs, path::PathBuf};
use analyzer_host::{AnalyzerHost, json_rpc::message::{Message, Request, Notification, Response}, lsp::workspace::{Workspace, WorkspaceManager}};
use cancellation::CancellationTokenSource;
use lsp_types::{Url, WorkspaceFolder, WorkspaceClientCapabilities, DidChangeWatchedFilesClientCapabilities};
use p4_analyzer::{driver::{Driver, buffer_driver::BufferStruct}, native_fs::native_fs::NativeFs};
use queues::*;
use crate::{flags::LsifP4Cmd, lsif_writer::{LsifWriter, LsifData}};

/// LsifGenerator job is to have all the components needed for the LSIF generation (it's the top level)
/// We achive this with LsifWriter, AnalyzerHost & Driver:
/// LsifWriter will deal with the JSon Serialization and file construction
/// AnalyzerHost contains infomation about the files & P4 Language
/// We can querry the CST that the host contains to make our own querries to generate LSP responses
/// The Driver is required for the AnalyzerHost to behaviour normally
/// It also has the aditional benifit of acting as a Client for the LSP, which is what the BufferDriver is
pub struct LsifGenerator {
    settings: Arc<LsifP4Cmd>,
    writer: LsifWriter,
    host: Option<Rc<AnalyzerHost>>,     // Both AnalyzerHost::Start() and Self need host
    driver: Option<Arc<BufferStruct>>,  // BufferStruct is past to a thread in Driver
    token: CancellationTokenSource,
}

impl LsifGenerator {
    pub fn new(settings: Arc<LsifP4Cmd>) -> Self {
        LsifGenerator{
            settings: settings.clone(),
            writer: LsifWriter::new(settings),
            driver: None,   // Initalized later
            host: None,     // Initalized later
            token: CancellationTokenSource::new(),
        }
    }
    
    fn data(&self) -> &LsifData { 
        &self.writer.data()
    }

    async fn read_workspace_manager(&self) -> WorkspaceManager {
        let host = self.host.as_ref().unwrap();
        let protocol_machine = host.protocol_machine.read().await;
        let protocol_machine_ref = protocol_machine.as_ref().unwrap();
        let state = protocol_machine_ref.state.read().await;
        let workspaces = state.workspaces().clone();

        workspaces
    }

    // Main method that will produce and write LSIF dump file once called
    pub async fn generate_dump(&mut self) {
        // setup everything
        let new_ref_driver = self.setup_env();
        let ref_host = self.host.as_mut().unwrap().clone();
        
        // async functions
        let host_func = ref_host.start(self.token.token().clone());
        let driver_func = new_ref_driver.start(self.token.token().clone());
        let task_func = self.async_start();

        // Meat of the program, all logic is from here
        let _ = tokio::join!(host_func, driver_func, task_func);

        // We've finished so write to file
        self.writer.write_file_to_disk();
    }

    // Setup for Analyzer core & driver
    fn setup_env(&mut self) -> Driver {
        self.driver = Some(Arc::new(BufferStruct::new(Queue::new())));

        let driver = Driver::new(p4_analyzer::driver::DriverType::Buffer(self.driver.as_ref().unwrap().clone()));

        self.host =
        Some(Rc::new(AnalyzerHost::new(driver.get_message_channel(), None, Some(Arc::new(NativeFs::new())))));

        driver
    }

    /// This function is required for tokio::join!() to run the correct functions in a certain order
    /// We want Host to start first, then driver, then our file generation code
    /// We also only want our file generation code to run sequenationally while none block for Host to return results
    async fn async_start(&mut self) {
        // Send Messages to get LSP to active_initialized state and links workspace files & system header files
        println!("Starting LSP Server");
        self.setup_workspace().await;
        println!("LSP Server Running");
        
        // Generation of LSIF data
        println!("Started generating LSIF file content");
        self.generate_hover().await;
        // *Add more LSIF features here*
        println!("Finished LSIF file content");
        
        // Closes Host and Driver 
        println!("Stopping LSP Server");
        self.stop_workspace().await;
        println!("Exitting");
    }

    // Tells Analyzer core the infomation needed for the workspace
    async fn setup_workspace(&mut self) {
        let mut initialize_params = lsp_types::InitializeParams{ ..Default::default() };

		// Adds the System head files as additional workspace
        initialize_params.workspace_folders = Some(vec![
            WorkspaceFolder{ uri: Url::from_directory_path(fs::canonicalize(&self.settings.workspace.clone().unwrap_or_else(|| PathBuf::from("."))).expect("Failed to find Workspace Folder")).unwrap(), name: "main_workspace".into() },
            WorkspaceFolder{ uri: Url::from_directory_path(fs::canonicalize(&self.settings.header_files.clone()).expect("Failed to find System Header Folder")).unwrap(), name: "system_headers".into() }
        ]);

        // Tells LSP we will handle filesystem watch & notify it
        // We won't and we don't care if a file changes as this is a snapshot and if the file changes the LSIF file become out of sync
        initialize_params.capabilities.workspace = Some(WorkspaceClientCapabilities{ ..Default::default() });
        initialize_params.capabilities.workspace.as_mut().unwrap().did_change_watched_files = Some(DidChangeWatchedFilesClientCapabilities{ ..Default::default() });
        
        // packages it
        let json = serde_json::json!(initialize_params);
		let initialize_request = Message::Request(Request{ id: 0.into(), method: String::from("initialize"), params: json });
        
        // initialized Notification
		let initialized_params = lsp_types::InitializedParams {};
		let json = serde_json::json!(initialized_params);
		let initialized_notification = Message::Notification(Notification{ method: String::from("initialized"), params: json });
        
        // add to queue & process
        self.driver.as_ref().unwrap().send_messages(queue![initialize_request, initialized_notification]).await;
        // results 
        let _ = self.driver.as_ref().unwrap().get_output_buffer(1).await;

        let initialized_response = Message::Response(Response{ id: 0.into(), result: Some(serde_json::json!(())), error: None });
        self.driver.as_ref().unwrap().send_messages(queue![initialized_response]).await;
        
        self.driver.as_ref().unwrap().wait_for_process_messages().await;

        // access workspace_manager
        // write the URLs to file
        let workspace_manager = self.read_workspace_manager().await;
        for workspace in workspace_manager.workspaces {
            for (url,_file) in workspace.1.files.read().unwrap().iter()  {
                self.writer.document(url);
            }
        }
    }

    async fn stop_workspace(&mut self) {
        self.driver.as_ref().unwrap().clear_both_buffers();
        // !None of these work to sucessfully shutdown the LSP
        // See branch af/lsp_shutdown_logging for logging messages
        // shutdown request
        /*let shutdown_request = Message::Request(Request{ id: 0.into(), method: String::from("shutdown"), params: "".into()});

        // Ask it to shutdown(doesn't work): send request
        self.driver.as_ref().unwrap().send_messages(queue![shutdown_request]).await;

        // Force it to shutdown(doesn't work): cancel token for extra security
        self.token.cancel();

        // Nuke it(doesn't work): delete memory
        drop(self.host.as_mut());
        self.host = None;
        */
        
        self.driver.as_ref().unwrap().wait_for_process_messages().await;

        // Write file, here is not ideal as some messages may not have been processed yet
        self.writer.write_file_to_disk();

        // Not ideal but works.
        std::process::exit(0);
    }

    async fn generate_hover(&mut self) {
        println!("Generating Hover data");
        self.driver.as_ref().unwrap().clear_both_buffers();

        // Template logic for generating LSIF file
        // Scan the parser CST for Function/variables names
        // 
        // Something like this:
        // self.host.parser.cst()
        // 
        // Make a LSP Message request(textDocument/hover) from the results
        // 
        // let hover_params = lsp_types::HoverParams { text_document_position_params: todo!(), work_done_progress_params: todo!() };
		// let json = serde_json::json!(hover_params);
		// let hover_request = Message::Request(Request{ id: 0.into(), method: String::from("textDocument/hover"), params: json });
        // 
        // send and receive the LSP response:
        // 
        // self.driver.as_ref().unwrap().send_messages(queue![initialize_request, initialized_notification]);
        // let result = self.driver.as_ref().unwrap().read_queue();
        // 
        // The response is serialized but matches the exact form for the LSIF 
        // https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#hover
        // https://microsoft.github.io/language-server-protocol/specifications/lsif/0.4.0/specification/#textDocument_hover
        // 
        // Add it to the file
        // self.writer.text_document_hover();
        //
        
        self.driver.as_ref().unwrap().clear_both_buffers();
    }
}
