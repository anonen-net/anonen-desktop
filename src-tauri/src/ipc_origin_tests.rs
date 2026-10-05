use std::collections::BTreeSet;

use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

const CAPTCHA_PAGE: &str = "https://anonen.net/captcha/?embed=1&theme=light";

fn app_page() -> &'static str {
    if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    }
}

#[tauri::command]
fn get_history_entries() -> &'static str {
    "reached"
}

#[tauri::command]
fn not_a_registered_command() -> &'static str {
    "reached"
}

fn window(label: &str) -> WebviewWindow<MockRuntime> {
    let app = mock_builder()
        .invoke_handler(tauri::generate_handler![
            get_history_entries,
            not_a_registered_command
        ])
        .build(tauri::generate_context!())
        .expect("the app builds with the real configuration");
    WebviewWindowBuilder::new(&app, label, Default::default())
        .build()
        .expect("the window can be created")
}

fn call(
    window: &WebviewWindow<MockRuntime>,
    command: &str,
    origin: &str,
) -> Result<String, String> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: origin.parse().expect("the origin is a URL"),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<String>().expect("the reply is a string"))
    .map_err(|error| error.to_string())
}

#[test]
fn the_app_page_can_call_a_registered_command() {
    let main = window("main");
    assert_eq!(
        call(&main, "get_history_entries", app_page()).as_deref(),
        Ok("reached")
    );
}

#[test]
fn the_overlay_can_call_a_registered_command() {
    let overlay = window("recording_overlay");
    assert_eq!(
        call(&overlay, "get_history_entries", app_page()).as_deref(),
        Ok("reached")
    );
}

#[test]
fn the_captcha_page_cannot_call_any_command() {
    let main = window("main");
    let refused = call(&main, "get_history_entries", CAPTCHA_PAGE);
    assert!(
        refused.is_err(),
        "a page outside the app reached a command: {refused:?}"
    );
}

#[test]
fn no_other_remote_origin_can_either() {
    let main = window("main");
    for origin in [
        "https://anonen.net",
        "https://example.com/",
        "http://localhost:8080/",
        "https://tauri.localhost.example.com/",
    ] {
        let refused = call(&main, "get_history_entries", origin);
        assert!(refused.is_err(), "{origin} reached a command: {refused:?}");
    }
}

#[test]
fn the_manifest_is_what_decides() {
    let main = window("main");
    let refused = call(&main, "not_a_registered_command", app_page());
    assert!(
        refused.is_err(),
        "an unlisted command was allowed: the app ACL manifest is not in effect"
    );
}

#[test]
fn a_window_without_the_capability_gets_nothing() {
    let stray = window("some_other_window");
    assert!(call(&stray, "get_history_entries", app_page()).is_err());
}

#[test]
fn every_command_the_frontend_can_call_is_allowed() {
    let manifests: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("OUT_DIR"),
        "/acl-manifests.json"
    )))
    .expect("acl-manifests.json is valid JSON");
    let allowed: BTreeSet<&str> = manifests["__app-acl__"]["permissions"]["app-commands"]
        ["commands"]["allow"]
        .as_array()
        .expect("app-commands lists the commands it allows")
        .iter()
        .filter_map(|name| name.as_str())
        .collect();

    let bindings = include_str!("../../src/bindings.ts");
    let called: BTreeSet<&str> = bindings
        .split("TAURI_INVOKE(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .collect();

    assert!(
        called.len() > 50,
        "bindings.ts was not read: {}",
        called.len()
    );
    let missing: Vec<&&str> = called.difference(&allowed).collect();
    assert!(
        missing.is_empty(),
        "the frontend can call commands that are not allowed: {missing:?}"
    );
}

#[test]
fn the_shipped_csp_refuses_to_be_framed_and_names_no_dev_server() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
    let security = &config["app"]["security"];

    let shipped = security["csp"].as_str().expect("csp is a string");
    assert!(shipped.contains("frame-ancestors 'self'"), "{shipped}");
    assert!(!shipped.contains("localhost:1420"), "{shipped}");

    assert!(
        shipped.contains("frame-src https://anonen.net;"),
        "{shipped}"
    );

    let dev = security["devCsp"].as_str().expect("devCsp is a string");
    assert!(dev.contains("frame-ancestors 'self'"), "{dev}");
    assert!(dev.contains("ws://localhost:1420"), "{dev}");
}
