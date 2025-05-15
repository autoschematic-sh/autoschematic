fn main() -> Result<(), Box<dyn std::error::Error>> {

    #[cfg(feature = "grpc")]
    let proto_files = &["proto/connector.proto"];

    #[cfg(feature = "grpc")]
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src/grpc_bridge")
        .compile(proto_files, &["proto"])?;
    
    println!("cargo:rerun-if-changed=proto/connector.proto");
    
    Ok(())
}
