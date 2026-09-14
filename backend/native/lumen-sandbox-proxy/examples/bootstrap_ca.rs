//! Bootstrap a CA into a directory. Used by `scripts/ca_interop_check.py`.
//!
//! Usage: `bootstrap_ca <directory> [key-bits]`

use std::path::PathBuf;

use lumen_sandbox_proxy::{CaBootstrap, FileCaStore};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let directory = PathBuf::from(args.next().ok_or("usage: bootstrap_ca <dir> [key-bits]")?);
    let key_bits: usize = args.next().map_or(Ok(4096), |value| value.parse())?;

    let materialized = CaBootstrap::new(
        FileCaStore::new(&directory),
        directory.join("mitmproxy-ca.pem"),
    )
    .with_key_size(key_bits)
    .ensure_ca()?;

    println!("{}", materialized.pem_path.display());
    Ok(())
}
