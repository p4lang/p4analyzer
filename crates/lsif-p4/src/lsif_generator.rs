use std::sync::Arc;

use analyzer_host::AnalyzerHost;
use p4_analyzer::driver::Driver;
use crate::{flags::LsifP4Cmd, lsif_writer::LsifWriter};

pub struct LsifGenerator {
    settings: Arc<LsifP4Cmd>,
    writer: LsifWriter,
    host: AnalyzerHost,
    driver: Driver,
}

impl LsifGenerator {
    // Redundent but keeps it consistent
    pub fn new(settings: Arc<LsifP4Cmd>) -> Self {
        LsifGenerator{
            settings: settings.clone(),
            writer: LsifWriter::new(settings),
            host: todo!(),
            driver: todo!()
        }
    }

    //? Could be static function in a Mod instead of Class method
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
