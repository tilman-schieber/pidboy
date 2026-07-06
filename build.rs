// Embed the browser-editor assets (built by scripts/build-editor.sh) into
// the binary for `pidc serve`. Missing assets become empty placeholders so
// plain `cargo build` / `cargo test` work without the wasm toolchain;
// `pidc serve` detects the empty wasm blob and tells the user what to run.

use std::env;
use std::fs;
use std::path::Path;

const ASSETS: &[&str] = &[
    "index.html",
    "editor.js",
    "pkg/pidc_wasm.js",
    "pkg/pidc_wasm_bg.wasm",
];

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let web = Path::new(&manifest_dir).join("editor/web");

    println!("cargo:rerun-if-changed=editor/web");

    for rel in ASSETS {
        let src = web.join(rel);
        let dst = Path::new(&out_dir).join(rel.replace('/', "_"));
        if src.exists() {
            fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {}: {}", src.display(), e));
        } else {
            fs::write(&dst, []).unwrap_or_else(|e| panic!("write {}: {}", dst.display(), e));
        }
    }
}
