use std::{sync::Arc, rc::Rc};
use analyzer_host::AnalyzerHost;
use cancellation::{CancellationToken, CancellationTokenSource};
use p4_analyzer::{driver::{Driver, buffer_driver::BufferStruct}, native_fs::native_fs::NativeFs};
use queues::*;
use crate::{flags::LsifP4Cmd, lsif_writer::LsifWriter};

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
    token: CancellationTokenSource
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
        self.setup_workspace();

        // Generation of LSIF data 
        self.generate_hover();

        // Closes Host and Driver 
        self.token.cancel();
    }

    // Tells Analyzer core the infomation needed for the workspace
    fn setup_workspace(&mut self) {

    }

    // Actual working functions for LSIF file
    fn generate_hover(&mut self) {

    }
}
