use std::fs;
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = "Generate OpenAPI JSON from current code")]
struct Args {
    /// Port used for server URL generation in the OpenAPI `servers` list.
    #[arg(long, default_value_t = 8000)]
    port: u16,

    /// Output file path.
    #[arg(long, default_value = "openapi.json")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Use the crate-local docs builder to generate the OpenAPI at runtime.
    // The crate name has a hyphen in Cargo.toml; Rust replaces '-' with '_' for the crate identifier.
    let doc = s_curve::docs::build_openapi(args.port)?;
    let s = serde_json::to_string_pretty(&doc)?;
    fs::write(&args.out, s)?;
    println!("wrote {}", args.out.display());
    Ok(())
}
