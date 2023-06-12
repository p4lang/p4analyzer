pub(crate) mod flags;
pub(crate) mod lsif_writer;
pub(crate) mod lsif_generator;

use std::sync::Arc;

use flags::LsifP4Cmd;
use lsif_generator::LsifGenerator;

pub fn main() {
	match LsifP4Cmd::from_env() {
		Ok(cmd) => {
			if cmd.version {
				println!(env!("CARGO_PKG_VERSION"));
				return;
			}

            let mut generator = LsifGenerator::new(Arc::new(cmd));
            generator.generate_dump();

            

        },
        Err(err) => {
			println!();
			println!("{}", err);
			println!();
		}
	}
}
