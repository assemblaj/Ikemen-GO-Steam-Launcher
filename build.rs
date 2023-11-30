use std::{env, path::PathBuf};
use std::io::Result; 

fn main() -> Result<()> {
    prost_build::compile_protos(&["proto/steam.proto"], &["proto"])?;

    tonic_build::configure()
        .build_client(false)
        .compile(&["proto/steam.proto"], &["proto"])
        .unwrap();

    Ok(())
}