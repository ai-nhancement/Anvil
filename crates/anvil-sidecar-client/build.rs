fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&[proto_root.join("anvil/v1/sidecar.proto")], &[&proto_root])?;
    Ok(())
}
