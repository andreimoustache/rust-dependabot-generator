use clap::Parser;
use walkdir::WalkDir;


#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}


fn main() {
    let args = Cli::parse();
    println!("Scanning directory {}.", args.path.to_str().unwrap());

    WalkDir::new(args.path)
    .follow_links(true)
    .into_iter()
    .filter_map(|entry| entry.ok())
    .collect::<()>();
}
