//! `forge-status <repo> <feature> [branch]`: print the review status JSON (read-only).
use speckit_forge::{github::GitHub, Forge};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: forge-status <repo> <feature> [branch]");
        std::process::exit(2);
    }
    let gh = GitHub::default();
    let t = std::time::Instant::now();
    let s = gh.status(std::path::Path::new(&a[1]), &a[2], a.get(3).map(String::as_str));
    eprintln!("status in {:?}", t.elapsed());
    println!("{}", serde_json::to_string_pretty(&s).unwrap());
}
