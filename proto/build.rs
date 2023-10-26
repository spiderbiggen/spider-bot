fn main() -> Result<(), Box<dyn std::error::Error>> {
    // trigger recompilation when a new migration is added
    println!("cargo:rerun-if-changed=protos");
    tonic_build::compile_protos("protos/api.v1.proto")?;
    Ok(())
}
