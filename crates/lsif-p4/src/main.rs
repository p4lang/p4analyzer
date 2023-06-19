pub(crate) mod flags;
pub(crate) mod lsif_writer;
pub(crate) mod lsif_generator;

use std::{sync::Arc, path::PathBuf};

use flags::LsifP4Cmd;
use lsif_generator::LsifGenerator;

// Can be run with command: cargo run --bin lsif-p4 -- -h .
#[tokio::main]
pub async fn main() {
	// This gets the arguments from the CLI
	match LsifP4Cmd::from_env() {
		Ok(mut cmd) => {
			if cmd.version {	// returns version
				println!(env!("CARGO_PKG_VERSION"));
				return;
			}
			cmd.header_files = Some(PathBuf::from("."));
			// Program is in this struct
            let mut generator = LsifGenerator::new(Arc::new(cmd));
            // Starts program
			generator.generate_dump().await;
			// LSIF file has been created, so exit
        },
        Err(err) => {
			println!("\n{}\n", err);
		}
	}
}
