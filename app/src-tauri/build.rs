fn main() {
    // The interface is embedded at compile time; rebuild when any file under app/ui changes.
    println!("cargo:rerun-if-changed=../ui");
    tauri_build::build()
}
