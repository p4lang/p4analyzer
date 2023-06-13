pub(crate) mod flags;
pub(crate) mod lsif_writer;
pub(crate) mod lsif_generator;

use std::sync::Arc;

use flags::LsifP4Cmd;
use lsif_generator::LsifGenerator;

// Can be run with command: cargo run --bin lsif-p4 -- -h .
#[tokio::main]
pub async fn main() {
	match LsifP4Cmd::from_env() {
		Ok(cmd) => {
			if cmd.version {
				println!(env!("CARGO_PKG_VERSION"));
				return;
			}

            let mut generator = LsifGenerator::new(Arc::new(cmd));
            generator.generate_dump().await;
        },
        Err(err) => {
			println!();
			println!("{}", err);
			println!();
		}
	}
}
