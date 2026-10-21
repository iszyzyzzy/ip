pub mod audio;
pub mod config;
pub mod control;
pub mod drivers;
pub mod h7;
pub mod server;
pub mod store;
pub mod validation;

pub mod eyes {
    #[allow(clippy::all)]
    pub mod v1 {
        tonic::include_proto!("eyes.v1");
        include!(concat!(env!("OUT_DIR"), "/eyes.v1.serde.rs"));
    }
}
pub mod robot {
    #[allow(clippy::all)]
    pub mod v1 {
        tonic::include_proto!("robot.v1");
        include!(concat!(env!("OUT_DIR"), "/robot.v1.serde.rs"));
    }
}
pub use robot::v1 as pb;

pub fn to_struct(value: &serde_json::Value) -> pbjson_types::Struct {
    serde_json::from_value(value.clone()).expect("设备状态必须为 JSON 对象")
}

pub fn from_struct(
    value: Option<pbjson_types::Struct>,
) -> Result<serde_json::Value, tonic::Status> {
    value
        .map(|v| serde_json::to_value(v).expect("Struct 可序列化"))
        .ok_or_else(|| tonic::Status::invalid_argument("缺少 config"))
}
