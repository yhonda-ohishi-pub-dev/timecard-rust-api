use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile proto files with reflection support
    tonic_build::configure()
        .build_server(true)
        .build_client(false) // No client needed for server-side
        .file_descriptor_set_path(out_dir.join("timecard_descriptor.bin"))
        .compile_protos(&["proto/timecard.proto"], &["proto"])?;

    Ok(())
}
