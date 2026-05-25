fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let src_gen_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/gen");
    let src_gen_file = src_gen_dir.join("anvil.v1.rs");

    println!(
        "cargo:rerun-if-changed={}",
        proto_root.join("anvil/v1/sidecar.proto").display()
    );
    println!("cargo:rerun-if-changed=src/gen/anvil.v1.rs");
    println!("cargo:rerun-if-env-changed=ANVIL_REGEN_PROTO");

    if std::env::var("ANVIL_REGEN_PROTO").is_ok() {
        // P3b handoff: change build_client(false) → build_client(true) to generate
        // the SidecarClient gRPC stub, then commit the updated src/gen/anvil.v1.rs.
        std::fs::create_dir_all(&src_gen_dir)?;
        tonic_build::configure()
            .build_server(false)
            .build_client(true)
            .out_dir(&src_gen_dir)
            .compile_protos(
                &[proto_root.join("anvil/v1/sidecar.proto")],
                &[&proto_root],
            )?;
    }

    let content = std::fs::read(&src_gen_file).map_err(|e| {
        format!("src/gen/anvil.v1.rs is missing ({e}). Run `just gen-rust` with protoc to regenerate.")
    })?;
    std::fs::write(out_dir.join("anvil.v1.rs"), content)?;
    Ok(())
}
