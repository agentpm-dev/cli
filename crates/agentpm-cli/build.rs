use std::fs;

fn main() {
    let canonical = "../../standards/agentpm/1.0.0/manifest.schema.json";
    let compatibility = "../../schemas/agentpm.manifest.schema.json";
    println!("cargo:rerun-if-changed={canonical}");
    println!("cargo:rerun-if-changed={compatibility}");
    assert_eq!(
        fs::read(canonical).expect("reading canonical APDS schema"),
        fs::read(compatibility).expect("reading generated compatibility schema"),
        "Manifest schema drift: edit standards/agentpm/1.0.0/manifest.schema.json and run python3 scripts/check_apds_schema.py --write"
    );
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
