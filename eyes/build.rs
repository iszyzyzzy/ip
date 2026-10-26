fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .message_attribute(".", "#[serde(default, deny_unknown_fields)]")
        .compile_with_config(config, &["proto/eyes.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/eyes.proto");
    Ok(())
}
