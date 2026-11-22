use std::{sync::LazyLock, time::Duration};

use prost::Message;
use prost_reflect::{DescriptorPool, DynamicMessage, SerializeOptions};
use robotd::pb::{self, robot_control_client::RobotControlClient};
use serde::Serialize;
use serde_json::Value;
use tonic::{Request, Status, transport::Channel};

static DESCRIPTORS: LazyLock<DescriptorPool> =
    LazyLock::new(|| DescriptorPool::decode(robotd::PROTO_DESCRIPTOR).expect("proto 描述符"));

#[derive(Clone)]
pub struct Robot {
    pub client: RobotControlClient<Channel>,
    token: Option<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>,
}

impl Robot {
    pub fn new(target: &str, token: Option<String>) -> anyhow::Result<Self> {
        let endpoint = if target.starts_with("http://") {
            target.into()
        } else {
            format!("http://{target}")
        };
        let channel = Channel::from_shared(endpoint)?.connect_lazy();
        Ok(Self {
            client: RobotControlClient::new(channel),
            token: token.map(|v| format!("Bearer {v}").parse()).transpose()?,
        })
    }

    pub fn request<M>(&self, msg: M) -> Request<M> {
        let mut req = Request::new(msg);
        if let Some(token) = &self.token {
            req.metadata_mut().insert("authorization", token.clone());
        }
        req
    }

    pub async fn call(&self, method: &str, value: Value) -> Result<Value, Status> {
        let mut client = self.client.clone();
        macro_rules! invoke {
            ($rpc:ident, $input:ident, $output:ident) => {{
                let mut request = self.request(input::<pb::$input>(stringify!($input), value)?);
                request.set_timeout(Duration::from_millis(if method == "drive" {
                    80
                } else {
                    500
                }));
                let response = client.$rpc(request).await?.into_inner();
                output(stringify!($output), &response)
            }};
        }
        match method {
            "get_state" => invoke!(get_state, Empty, RobotState),
            "get_config" => invoke!(get_config, Empty, ConfigSnapshot),
            "update_config" => invoke!(update_config, ConfigUpdate, ConfigSnapshot),
            "update_eyes" => invoke!(update_eyes, EyesUpdate, ConfigSnapshot),
            "list_actions" => invoke!(list_actions, Empty, ActionList),
            "list_sounds" => invoke!(list_sounds, Empty, SoundList),
            "put_action" => invoke!(put_action, ActionUpdate, ActionRecord),
            "delete_action" => invoke!(delete_action, ActionDelete, Ack),
            "capture_preset" => invoke!(capture_preset, PresetCapture, ActionRecord),
            "acquire" => invoke!(acquire_control, AcquireRequest, Lease),
            "renew" => invoke!(renew_control, LeaseRequest, Lease),
            "release" => invoke!(release_control, LeaseRequest, Ack),
            "drive" | "targets" => invoke!(set_targets, TargetRequest, Ack),
            "stop" => invoke!(stop, StopRequest, Ack),
            "run_action" => invoke!(run_action, RunRequest, Ack),
            "cancel_action" => invoke!(cancel_action, LeaseRequest, Ack),
            "zero_neck" => invoke!(zero_neck, ZeroRequest, Ack),
            "get_audio_state" => invoke!(get_audio_state, Empty, AudioStatus),
            _ => Err(Status::not_found("未知命令")),
        }
    }
}

fn descriptor(name: &str) -> prost_reflect::MessageDescriptor {
    DESCRIPTORS
        .get_message_by_name(&format!("robot.v1.{name}"))
        .expect("RPC 消息描述符")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn action_sequence_curve_round_trips_through_wan_json() {
        let mut store = robotd::store::Store::open(":memory:", None).unwrap();
        for curve in ["SERVO_NATIVE", "S_CURVE", "S_CURVE_STRONG"] {
            let request = json!({"action":{"name":curve,"sequence":{
                "motion_curve":curve,"steps":[{"event":{"sound":"问候"}},{"preset":"2"},{"event":{"eye_expression":"NORMAL"}}]
            }}});
            let update = input::<pb::ActionUpdate>("ActionUpdate", request).unwrap();
            let action = update.action.unwrap();
            let revision = store.put(&action, update.expected_revision).unwrap();
            let record = pb::ActionRecord {
                action: Some(store.action(curve).unwrap()),
                revision,
                deleted: false,
            };
            let result = output("ActionRecord", &record).unwrap();
            assert_eq!(result["action"]["sequence"]["motion_curve"], curve);
            let steps = &result["action"]["sequence"]["steps"];
            assert_eq!(steps[0]["event"]["sound"], "问候");
            assert_eq!(steps[2]["event"]["eye_expression"], "NORMAL");
        }
    }
}

fn input<M: Message + Default>(name: &str, value: Value) -> Result<M, Status> {
    let msg = DynamicMessage::deserialize(descriptor(name), value)
        .map_err(|e| Status::invalid_argument(e.to_string()))?;
    M::decode(msg.encode_to_vec().as_slice()).map_err(|e| Status::invalid_argument(e.to_string()))
}

fn output<M: Message>(name: &str, value: &M) -> Result<Value, Status> {
    let msg = DynamicMessage::decode(descriptor(name), value.encode_to_vec().as_slice())
        .map_err(|e| Status::internal(e.to_string()))?;
    let options = SerializeOptions::new()
        .use_proto_field_name(true)
        .skip_default_fields(false);
    msg.serialize_with_options(serde_json::value::Serializer, &options)
        .map_err(|e| Status::internal(e.to_string()))
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<Status> for ApiError {
    fn from(value: Status) -> Self {
        let code = match value.code() {
            tonic::Code::InvalidArgument => "INVALID_ARGUMENT",
            tonic::Code::NotFound => "NOT_FOUND",
            tonic::Code::Unavailable => "UNAVAILABLE",
            tonic::Code::PermissionDenied => "PERMISSION_DENIED",
            tonic::Code::Unauthenticated => "UNAUTHENTICATED",
            tonic::Code::Aborted => "ABORTED",
            tonic::Code::FailedPrecondition => "FAILED_PRECONDITION",
            tonic::Code::DeadlineExceeded => "DEADLINE_EXCEEDED",
            tonic::Code::ResourceExhausted => "RESOURCE_EXHAUSTED",
            tonic::Code::AlreadyExists => "ALREADY_EXISTS",
            tonic::Code::Cancelled => "CANCELLED",
            _ => "INTERNAL",
        };
        Self::new(code, value.message())
    }
}
