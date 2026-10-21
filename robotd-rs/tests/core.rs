use robotd::{
    config::Config,
    drivers::{chassis_payload, decode_chassis},
    h7::{Packet, crc},
    pb,
    store::Store,
    validation,
};
use serde_json::json;

#[test]
fn protocol_crc_cobs_and_ranger_vectors() {
    assert_eq!(crc(b"123456789"), 0x29b1);
    for len in 0..=80 {
        for byte in [0, 255, 17] {
            let p = Packet {
                kind: 4,
                channel: 3,
                flags: 13,
                seq: 0xa100 + len,
                payload: vec![byte; len as usize],
            };
            let wire = p.encode();
            assert_eq!(Packet::decode(&wire[..wire.len() - 1]).unwrap(), p);
            let mut corrupt = wire.clone();
            corrupt[2] ^= 1;
            assert!(Packet::decode(&corrupt[..corrupt.len() - 1]).is_err());
        }
    }
    assert_eq!(
        chassis_payload(&pb::ChassisTarget {
            speed_m_s: 0.15,
            ..Default::default()
        }),
        [0, 150, 0, 0, 0, 0, 0, 0]
    );
    let (_, v) = decode_chassis(0x211, &[0, 1, 0, 240, 0, 0, 0, 128]).unwrap();
    assert_eq!(v["voltage_v"], 24.0);
    assert_eq!(v["fault_bits"], 128);
}

#[test]
fn config_validation_and_store_compare_and_swap() {
    let mut c = Config::defaults();
    c.left_arm.joints[1].id = 0;
    assert!(c.validate().is_err());
    c = Config::defaults();
    c.audio.prebuffer_ms = 1000;
    assert!(c.validate().is_err());
    c = Config::defaults();
    c.eyes.settings["jelly"]["frequency"] = json!(100);
    assert!(c.validate().is_err());
    let store = Store::open(":memory:", None).unwrap();
    let (rev, c) = store.config().unwrap();
    assert_eq!(rev, 1);
    assert_eq!(store.update(1, &c).unwrap(), 2);
    assert_eq!(
        store.update(1, &c).unwrap_err().code(),
        tonic::Code::Aborted
    );
    let t = pb::Targets {
        neck: Some(pb::NeckTarget {
            yaw_rad: Some(f64::NAN),
            pitch_rad: None,
        }),
        ..Default::default()
    };
    assert!(validation::targets(&t, &c).is_err());
}

#[test]
fn persisted_action_json_and_delete_revision() {
    let mut store = Store::open(":memory:", None).unwrap();
    // 旧版 Python 写入 snake_case 和枚举字符串，仍应按原协议读取。
    let a: pb::Action =
        serde_json::from_value(json!({"name":"test","duration_ms":500,"steps":[{"at_ms":0,
        "targets":{"chassis":{"mode":"ACKERMANN","speed_m_s":0.1,"ttl_ms":200}}}]}))
        .unwrap();
    validation::action(&a, &Config::defaults()).unwrap();
    assert_eq!(store.put(&a, 0).unwrap(), 1);
    assert_eq!(store.action("test").unwrap(), a);
    assert_eq!(store.delete("test", 1).unwrap(), 2);
    assert!(store.actions().unwrap().actions[0].deleted);
    assert!(store.put(&a, 0).is_err());
    assert_eq!(store.put(&a, 2).unwrap(), 3);
}
