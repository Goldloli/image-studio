//! Guards the Tauri command surface: every invoke handler must be declared in
//! the build manifest and allowed by the main-window capability. A command that
//! exists in `generate_handler!` but is missing either declaration compiles and
//! passes unit tests, yet the webview call is rejected at runtime.

use std::fs;

#[test]
fn every_invoke_handler_is_declared_and_allowed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = fs::read_to_string(root.join("src/host.rs")).expect("host source");
    let marker = "generate_handler![";
    let start = host.find(marker).expect("invoke handler list");
    let tail = &host[start + marker.len()..];
    let end = tail.find(']').expect("handler list terminator");
    let commands = tail[..end]
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    assert!(
        commands.len() > 10,
        "handler list parse produced too few commands"
    );

    let build = fs::read_to_string(root.join("build.rs")).expect("build script");
    let capability_source =
        fs::read_to_string(root.join("capabilities/main.json")).expect("capability manifest");
    let capability: serde_json::Value =
        serde_json::from_str(&capability_source).expect("capability manifest is valid JSON");
    let permissions = capability["permissions"]
        .as_array()
        .expect("capability permissions array");

    for command in commands {
        assert!(
            build.contains(&format!("\"{command}\"")),
            "build.rs 未登记命令 {command}（Tauri 权限不会生成）"
        );
        let permission = format!("allow-{}", command.replace('_', "-"));
        assert!(
            permissions
                .iter()
                .any(|value| value.as_str() == Some(permission.as_str())),
            "capabilities/main.json 未允许 {permission}（运行时会被 ACL 拒绝）"
        );
    }
}
