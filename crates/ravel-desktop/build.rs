fn main() {
    // include_dir embeds assets; track directory changes on stable Rust as well
    // so CSS/JS/font updates always invalidate an existing desktop build.
    println!("cargo:rerun-if-changed=assets/markdown");
}
