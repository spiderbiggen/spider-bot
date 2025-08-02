fn main() -> Result<(), Box<dyn std::error::Error>> {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=protos");
    tonic_prost_build::configure()
        .emit_rerun_if_changed(true)
        .compile_protos(&["protos/api.v2.proto"], &["protos"])?;
    Ok(())
}
