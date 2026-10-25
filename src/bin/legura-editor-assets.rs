fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let json = arguments.len() == 3 && arguments[2] == "--json";
    if (arguments.len() == 2 || json) && (arguments[0] == "check" || arguments[0] == "install") {
        let project = std::path::Path::new(&arguments[1]);
        let result = if arguments[0] == "check" {
            legura::editor_install::check(project)
        } else {
            legura::editor_install::install(project)
        };
        match result {
            Ok(report) if json => println!("{}", serde_json::to_string(&report).expect("installation report")),
            Ok(report) => println!(
                "Generation: {}\nInstalled: {}\nComponent: {}\nStylesheet: {}\nModule: {}\nActivation is separate; no imports, configuration or database were changed.",
                report.generation, report.installed, report.component, report.stylesheet, report.module
            ),
            Err(error) => {
                eprintln!("Editor installation refused: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if arguments.len() != 1 || arguments[0] == "check" || arguments[0] == "install" {
        eprintln!("Usage: legura-editor-assets <new-bundle-directory> | check <project> [--json] | install <project> [--json]");
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
