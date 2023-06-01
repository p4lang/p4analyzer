use std::path::PathBuf;

xflags::xflags! {
	src "./src/cli/flags.rs"

	/// A Language Server Protocol (LSP) implementation for the P4 programming
	/// language (see https://p4.org).
	cmd p4-analyzer {
		/// Optional path to a folder where a log file will be written.
		optional --logpath path: PathBuf

		/// Optional log level to apply when writing to the log file. Defaults to 'debug'.
		optional --loglevel level: String

		///  Displays the version number.
		optional -v,--version

        /// Optional flag for choosing the native filesystem instead of LSP Requsts
        optional -n,--nativefile

		/// Starts executing the LSP server (default command).
		default cmd server {
			/// Use the 'stdio' transport (default).
			optional --stdio
		}
	}
}
// generated start
// The following code is generated by `xflags` macro.
// Run `env UPDATE_XFLAGS=1 cargo build` to regenerate.
#[derive(Debug)]
pub struct P4Analyzer {
    pub logpath: Option<PathBuf>,
    pub loglevel: Option<String>,
    pub version: bool,
    pub nativefile: bool,
    pub subcommand: P4AnalyzerCmd,
}

#[derive(Debug)]
pub enum P4AnalyzerCmd {
    Server(Server),
}

#[derive(Debug)]
pub struct Server {
    pub stdio: bool,
}

impl P4Analyzer {
    #[allow(dead_code)]
    pub fn from_env_or_exit() -> Self {
        Self::from_env_or_exit_()
    }

    #[allow(dead_code)]
    pub fn from_env() -> xflags::Result<Self> {
        Self::from_env_()
    }

    #[allow(dead_code)]
    pub fn from_vec(args: Vec<std::ffi::OsString>) -> xflags::Result<Self> {
        Self::from_vec_(args)
    }
}
// generated end
