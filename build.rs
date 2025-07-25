use prost_build::Config;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/proto");
    
    // Check if protoc is available
    if let Err(_) = std::process::Command::new("protoc").arg("--version").output() {
        println!("cargo:warning=protoc not found, skipping protobuf compilation");
        println!("cargo:warning=Install protoc to enable full protobuf support");
        return Ok(());
    }
    
    // Configure prost
    let mut config = Config::new();
    config.out_dir("src/proto/generated");
    
    // Ensure output directory exists
    fs::create_dir_all("src/proto/generated")?;
    
    // Compile core protobuf files
    let proto_files = [
        "src/proto/wa_common/WACommon.proto",
        "src/proto/wa_web/WAWebProtobufsWeb.proto", 
        "src/proto/wa_e2e/WAWebProtobufsE2E.proto",
        "src/proto/wa_msg_transport/WAMsgTransport.proto",
        "src/proto/wa_multi_device/WAMultiDevice.proto",
        "src/proto/wa_companion_reg/WACompanionReg.proto",
    ];
    
    // Only compile files that exist
    let existing_files: Vec<&str> = proto_files
        .iter()
        .filter(|file| Path::new(file).exists())
        .copied()
        .collect();
    
    if existing_files.is_empty() {
        println!("cargo:warning=No protobuf files found to compile");
        return Ok(());
    }
    
    println!("cargo:warning=Compiling {} protobuf files", existing_files.len());
    
    // Compile the protobuf files
    config.compile_protos(&existing_files, &["src/proto"])?;
    
    println!("cargo:warning=Protobuf compilation completed successfully");
    Ok(())
}