use std::{env, fs, path::PathBuf};

fn main() {
    let source = "platforms/targets.json";
    println!("cargo:rerun-if-changed={source}");
    println!(
        "cargo:rustc-env=AGENTPM_BUILD_TARGET={}",
        env::var("TARGET").unwrap()
    );
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(source).unwrap()).unwrap();
    let targets = value["targets"].as_array().unwrap();
    let mut generated = String::from(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum Target { Any,",
    );
    for target in targets {
        generated.push_str(&format!("{},", target["variant"].as_str().unwrap()));
    }
    generated.push_str("}\nimpl Target { pub const SUPPORTED: &'static [Self] = &[");
    for target in targets {
        generated.push_str(&format!("Self::{},", target["variant"].as_str().unwrap()));
    }
    generated
        .push_str("] ; pub fn as_str(self) -> &'static str { match self { Self::Any => \"any\",");
    for target in targets {
        generated.push_str(&format!(
            "Self::{} => {:?},",
            target["variant"].as_str().unwrap(),
            target["id"].as_str().unwrap()
        ));
    }
    generated.push_str("}} pub fn marker(self) -> &'static str { match self { Self::Any => \"\",");
    for target in targets {
        generated.push_str(&format!(
            "Self::{} => {:?},",
            target["variant"].as_str().unwrap(),
            target["marker"].as_str().unwrap()
        ));
    }
    generated.push_str("}} }");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("targets.rs"),
        generated,
    )
    .unwrap();
}
