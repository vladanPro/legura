fn main() {
    println!("cargo:rerun-if-changed=actions.ax");
    let source = std::fs::read_to_string("actions.ax").expect("action fixture must exist");
    let module = axonyx_core::ax_backend_codegen::compile_backend_ax_to_module(&source)
        .expect("native content action must compile");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let backend = output.join("backend.rs");
    std::fs::write(&backend, module).expect("generated fixture must be writable");
    std::fs::write(
        output.join("module.rs"),
        format!("#[path = {backend:?}] mod backend;"),
    )
    .expect("module wrapper must be writable");
}
