//! Run only against the app's disposable target/asset-smoke pack.
use eyes::control::{connect, pb};
use std::{path::PathBuf, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::args().nth(1).expect("socket path");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/asset-smoke");
    let json_path = root.join("eye.json");
    let shader_path = root.join("eyes.wgsl");
    let original_json = std::fs::read(&json_path)?;
    let original_shader = std::fs::read_to_string(&shader_path)?;
    let mut client = connect(socket).await?;
    let mut config: serde_json::Value = serde_json::from_slice(&original_json)?;
    // 默认画稿会变化，回滚应恢复加载前的配置，而不是固定旧版本尺寸。
    let original_size = config["defaults"]["iris_size"].as_f64().unwrap() as f32;
    config["defaults"]["iris_size"] = serde_json::json!(0.18);
    config["roundness"] = serde_json::json!(0.5);
    std::fs::write(&json_path, serde_json::to_vec_pretty(&config)?)?;
    client.reload_assets(pb::Empty {}).await?;
    assert_eq!(
        client
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .config
            .unwrap()
            .iris_size,
        Some(0.18)
    );
    // A broken shader must keep the last complete pack, not crash the window.
    std::fs::write(&shader_path, "invalid WGSL")?;
    assert_eq!(
        client.reload_assets(pb::Empty {}).await.unwrap_err().code(),
        tonic::Code::FailedPrecondition
    );
    assert_eq!(
        client
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .config
            .unwrap()
            .iris_size,
        Some(0.18)
    );
    // Syntactically valid but incompatible GPU binding exercises error scopes.
    std::fs::write(
        &shader_path,
        original_shader.replace("@binding(2)", "@binding(7)"),
    )?;
    assert_eq!(
        client.reload_assets(pb::Empty {}).await.unwrap_err().code(),
        tonic::Code::FailedPrecondition
    );
    std::fs::write(&shader_path, &original_shader)?;
    // Exercise the alternative vector loader and defaults in the live renderer.
    config["textures"][0] = serde_json::json!("iris.svg");
    std::fs::write(&json_path, serde_json::to_vec_pretty(&config)?)?;
    client.reload_assets(pb::Empty {}).await?;
    std::fs::write(&json_path, &original_json)?;
    client.reload_assets(pb::Empty {}).await?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        client
            .get_state(pb::Empty {})
            .await?
            .into_inner()
            .config
            .unwrap()
            .iris_size,
        Some(original_size)
    );
    println!(
        "PASS: external JSON reload, PNG and SVG layers, shader error rollback, GPU layout error rollback, recovery"
    );
    Ok(())
}
