fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptor = out.join("robot.bin");
    let mut prost = tonic_prost_build::Config::new();
    prost.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure()
        .file_descriptor_set_path(&descriptor)
        .compile_well_known_types(true)
        .extern_path(".google.protobuf", "::pbjson_types")
        .compile_with_config(
            prost,
            &["../robotd/proto/robot.proto", "../robotd/pb/eyes.proto"],
            &["../robotd/proto", "../robotd/pb"],
        )?;
    pbjson_build::Builder::new()
        .register_descriptors(&std::fs::read(descriptor)?)?
        .build(&[".robot.v1", ".eyes.v1"])?;
    println!("cargo:rerun-if-changed=../robotd/proto/robot.proto");
    println!("cargo:rerun-if-changed=../robotd/pb/eyes.proto");
    Ok(())
}
