use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=../plugin/src");
    println!("cargo:rerun-if-changed=../plugin/Cargo.toml");
    println!("cargo:rerun-if-changed=../status-bar/src");
    println!("cargo:rerun-if-changed=../status-bar/Cargo.toml");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    // Build/fetch bunshin plugin
    build_or_fetch_plugin(
        "bunshin",
        "../plugin",
        &out_dir.join("bunshin.wasm"),
    )?;

    // Build/fetch status-bar plugin
    build_or_fetch_plugin(
        "status-bar",
        "../status-bar",
        &out_dir.join("status-bar.wasm"),
    )?;

    Ok(())
}

fn build_or_fetch_plugin(
    plugin_name: &str,
    plugin_dir: &str,
    wasm_dest: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if we have a pre-built WASM (for releases)
    let prebuilt_wasm = PathBuf::from(format!("{}/prebuilt/{}.wasm", plugin_dir, plugin_name));

    if prebuilt_wasm.exists() {
        // Use pre-built WASM (release builds, CI)
        println!(
            "cargo:warning=Using pre-built WASM plugin {} from {}",
            plugin_name,
            prebuilt_wasm.display()
        );
        std::fs::copy(&prebuilt_wasm, wasm_dest)?;
        println!(
            "cargo:warning=Pre-built {} WASM copied to: {}",
            plugin_name,
            wasm_dest.display()
        );
    } else {
        // Build WASM on-the-fly (development)
        println!("cargo:warning=Building {} plugin to WASM...", plugin_name);

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
                &format!("{}/Cargo.toml", plugin_dir),
            ])
            .status()?;

        if !status.success() {
            return Err(format!("Failed to build {} plugin WASM", plugin_name).into());
        }

        // Copy the WASM file to the output directory for embedding
        let wasm_src = PathBuf::from(format!(
            "{}/target/wasm32-wasip1/release/{}.wasm",
            plugin_dir, plugin_name
        ));

        if wasm_src.exists() {
            std::fs::copy(&wasm_src, wasm_dest)?;
            println!(
                "cargo:warning={} WASM plugin built and copied to: {}",
                plugin_name,
                wasm_dest.display()
            );
        } else {
            return Err(format!("WASM file not found at: {}", wasm_src.display()).into());
        }
    }

    Ok(())
}
