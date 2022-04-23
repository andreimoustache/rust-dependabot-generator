use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[clap(parse(from_os_str))]
    path: std::path::PathBuf,
}


fn main() {
    let args = Cli::parse();
    println!("Scanning directory {}.", args.path.to_str().unwrap());
}
