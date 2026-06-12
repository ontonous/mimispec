use clap::Parser;
use mimispec::parse;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mimispec", version, about = "MimiSpec parser CLI")]
struct Args {
    /// .mms file to parse
    #[arg(default_value = "-")]
    file: PathBuf,

    /// Show AST structure
    #[arg(short, long)]
    ast: bool,
}

fn main() {
    let args = Args::parse();

    let source = if args.file == PathBuf::from("-") {
        // Read from stdin
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap_or_default();
        input
    } else {
        fs::read_to_string(&args.file).expect("Failed to read file")
    };

    let result = parse(&source);

    if result.errors.is_empty() {
        println!("✓ Parsing successful");
        if args.ast {
            println!("{:#?}", result.file);
        }
    } else {
        println!("✗ Parsing failed with {} error(s)", result.errors.len());
        for err in &result.errors {
            println!("  - {:?}", err);
        }
        std::process::exit(1);
    }
}