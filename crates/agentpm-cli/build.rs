use std::fs;

fn main() {
    let source = "../../platforms/uv-pins.json";
    println!("cargo:rerun-if-changed={source}");
    let pins: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(source).unwrap()).unwrap();
    let version = pins["version"].as_str().unwrap();
    assert!(
        version.split('.').count() == 3
            && version
                .split('.')
                .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    );
    println!("cargo:rustc-env=AGENTPM_UV_VERSION={version}");
}
