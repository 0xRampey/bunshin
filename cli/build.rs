use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../plugin/src");
    println!("cargo:rerun-if-changed=../plugin/Cargo.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let wasm_dest = out_dir.join("bunshin.wasm");

    // Check if we have a pre-built WASM (for releases)
    let prebuilt_wasm = PathBuf::from("../plugin/prebuilt/bunshin.wasm");

    if prebuilt_wasm.exists() {
        // Use pre-built WASM (release builds, CI)
        println!("cargo:warning=Using pre-built WASM plugin from plugin/prebuilt/");
        std::fs::copy(&prebuilt_wasm, &wasm_dest)?;
        println!("cargo:warning=Pre-built WASM copied to: {}", wasm_dest.display());
    } else {
        // Build WASM on-the-fly (development)
        println!("cargo:warning=Building bunshin plugin to WASM...");

        // Check if wasm32-wasip1 target is installed
        let target_check = Command::new("rustup")
            .args(&["target", "list", "--installed"])
            .output()?;

        let targets = String::from_utf8_lossy(&target_check.stdout);
        if !targets.contains("wasm32-wasip1") {
            println!("cargo:warning=wasm32-wasip1 target not installed!");
            println!("cargo:warning=Run: rustup target add wasm32-wasip1");
            return Err("Missing wasm32-wasip1 target".into());
        }

        let status = Command::new("cargo")
            .args(&[
                "build",
                "--release",
                "--target",
                "wasm32-wasip1",
                "--manifest-path",
                "../plugin/Cargo.toml",
            ])
            .status()?;

        if !status.success() {
            return Err("Failed to build plugin WASM".into());
        }

        // Copy the WASM file to the output directory for embedding
        let wasm_src = PathBuf::from("../plugin/target/wasm32-wasip1/release/bunshin.wasm");

        if wasm_src.exists() {
            std::fs::copy(&wasm_src, &wasm_dest)?;
            println!("cargo:warning=WASM plugin built and copied to: {}", wasm_dest.display());
        } else {
            return Err(format!("WASM file not found at: {}", wasm_src.display()).into());
        }
    }

    Ok(())
}
