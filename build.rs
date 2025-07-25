fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For now, skip protobuf compilation
    // TODO: Add proper protobuf support when protoc is available
    println!("cargo:warning=Skipping protobuf compilation for now");
    println!("cargo:rerun-if-changed=whatsmeow-go/proto");
    Ok(())
}