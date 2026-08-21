use curia::Level;
use tauri_plugin_curia::WebviewRecord;

#[test]
fn nested_values_and_types_survive_the_boundary() {
    let raw = serde_json::json!({
        "frames": 1024,
        "live": true,
        "detail": { "ok": true, "codes": [1, 2, 3] }
    });
    let serde_json::Value::Object(map) = raw else {
        unreachable!()
    };

    let record = WebviewRecord {
        level: Level::Warn,
        message: "capture failed".to_string(),
        fields: Some(map),
        location: Some("dashboard".to_string()),
        file: None,
        line: None,
    };

    let event = record.into_event();

    assert_eq!(event.level, Level::Warn);
    assert_eq!(event.message, "capture failed");
    assert_eq!(
        event.fields.get("frames").unwrap(),
        &serde_json::json!(1024)
    );
    assert_eq!(event.fields.get("live").unwrap(), &serde_json::json!(true));
    assert_eq!(
        event.fields.get("detail").unwrap(),
        &serde_json::json!({ "ok": true, "codes": [1, 2, 3] })
    );
}

#[test]
fn the_location_becomes_the_target() {
    let record = WebviewRecord {
        level: Level::Info,
        message: "hello".to_string(),
        fields: None,
        location: Some("PlayerManager".to_string()),
        file: None,
        line: None,
    };

    assert_eq!(record.into_event().target, "webview:PlayerManager");
}

#[test]
fn a_missing_location_falls_back_to_the_bare_target() {
    let record = WebviewRecord {
        level: Level::Info,
        message: "hello".to_string(),
        fields: None,
        location: None,
        file: None,
        line: None,
    };

    assert_eq!(record.into_event().target, "webview");
}
