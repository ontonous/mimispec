use clap::Parser;
use mimispec::parse;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mimispec", version, about = "MimiSpec parser CLI")]
struct Args {
    /// .mms file(s) to parse; use - for stdin
    #[arg(default_value = "-")]
    files: Vec<PathBuf>,

    /// Show AST structure
    #[arg(short, long)]
    ast: bool,
}

fn parse_one(path: &PathBuf, ast: bool) -> (bool, usize) {
    let source = if path == &PathBuf::from("-") {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap_or_default();
        input
    } else {
        match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                println!("✗ Failed to read {}: {}", path.display(), e);
                return (false, 1);
            }
        }
    };

    let result = parse(&source);

    if result.errors.is_empty() {
        println!("✓ Parsing successful: {}", path.display());
        if ast {
            println!("{:#?}", result.file);
        }
        (true, 0)
    } else {
        println!(
            "✗ Parsing failed for {} with {} error(s)",
            path.display(),
            result.errors.len()
        );
        for err in &result.errors {
            println!("  - {:?}", err);
        }
        (false, result.errors.len())
    }
}

fn main() {
    let args = Args::parse();

    let mut total_errors = 0usize;
    let mut any_failure = false;

    // 串行处理每个文件，避免并行占用过高
    for path in &args.files {
        let (ok, errs) = parse_one(path, args.ast);
        if !ok {
            any_failure = true;
        }
        total_errors += errs;
    }

    if any_failure {
        eprintln!("\nTotal error(s): {}", total_errors);
        std::process::exit(1);
    }
}
