fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 1 {
        eprintln!("Usage: legura-editor-assets <new-bundle-directory>");
        std::process::exit(2);
    }
    match legura::editor_assets::export_bundle(std::path::Path::new(&arguments[0])) {
        Ok(_) => println!("Exported pinned editor bundle; see bundle.json. Existing directories are never overwritten."),
        Err(error) => {
            eprintln!("Editor bundle export failed: {error}");
            std::process::exit(1);
        }
    }
}
