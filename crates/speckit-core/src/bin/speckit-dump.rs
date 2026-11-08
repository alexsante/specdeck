//! Print a Spec Kit repository as JSON: `speckit-dump <repo> [out.json]`.

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(repo) = args.next() else {
        eprintln!("usage: speckit-dump <repo> [out.json]");
        std::process::exit(2);
    };
    let started = std::time::Instant::now();
    let project = match speckit_core::read_project(std::path::Path::new(&repo)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let json = serde_json::to_string_pretty(&project).expect("serialize");
    match args.next() {
        Some(out) => std::fs::write(&out, json).expect("write output"),
        None => println!("{json}"),
    }
    eprintln!("{} features in {:?}", project.features.len(), started.elapsed());
}
