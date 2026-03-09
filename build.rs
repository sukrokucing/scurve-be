use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Re-run if permissions.json changes
    println!("cargo:rerun-if-changed=permissions.json");
    // Re-run if migrations change so `sqlx::migrate!()` embeds latest files
    println!("cargo:rerun-if-changed=migrations");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("permissions_generated.rs");

    let permissions_json =
        fs::read_to_string("permissions.json").expect("Failed to read permissions.json");

    let permissions: Vec<serde_json::Value> =
        serde_json::from_str(&permissions_json).expect("Failed to parse permissions.json");

    let mut content = String::from("/// Generated permission constants\n");

    for perm in permissions {
        let name = perm["name"].as_str().unwrap();
        let value = perm["value"].as_str().unwrap();
        content.push_str(&format!("    pub const {}: &str = \"{}\";\n", name, value));
    }

    fs::write(&dest_path, content).expect("Failed to write permissions_generated.rs");
}
