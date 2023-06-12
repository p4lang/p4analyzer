use std::sync::Arc;

use analyzer_host::AnalyzerHost;
use cancellation::{CancellationToken, CancellationTokenSource};
use p4_analyzer::driver::{Driver, buffer_driver::BufferStruct};
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
    host: Option<AnalyzerHost>,
    driver: Option<BufferStruct>,
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
    pub fn generate_dump(&mut self) {
        // setup everything
        self.setup_env();
        self.setup_workspace();

        // Generation of LSIF data 
        self.generate_hover();

        // Write to file
        self.writer.write_file_to_disk();
    }

    // Setup for Analyzer core & driver
    fn setup_env(&mut self) {

    }

    // Tells Analyzer core the infomation needed for the workspace
    fn setup_workspace(&mut self) {

    }

    // Actual working functions for LSIF file
    fn generate_hover(&mut self) {

    }
}
