use std::fs;

fn main() -> anyhow::Result<()> {
    // Use the crate-local docs builder to generate the OpenAPI at runtime.
    // The crate name has a hyphen in Cargo.toml; Rust replaces '-' with '_' for the crate identifier.
    let doc = s_curve::docs::build_openapi(8000)?;
    let s = serde_json::to_string_pretty(&doc)?;
    let path = "/tmp/openapi-debug-generated.json";
    fs::write(path, s)?;
    println!("wrote {}", path);
    Ok(())
}
