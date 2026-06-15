use clap::Parser;
use mimispec::parse;
use serde::Serialize;
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

    /// Output results as JSON (useful for editor integrations)
    #[arg(short, long)]
    json: bool,

    /// Render AST back to MimiSpec source
    #[arg(short, long)]
    render: bool,
}

#[derive(Serialize)]
struct JsonError {
    line: usize,
    col: usize,
    message: String,
}

#[derive(Serialize)]
struct JsonResult {
    path: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render: Option<String>,
    errors: Vec<JsonError>,
}

#[derive(Serialize)]
struct JsonOutput {
    results: Vec<JsonResult>,
}

fn parse_one(path: &PathBuf, ast: bool, json: bool, render: bool) -> (bool, usize, JsonResult) {
    let source = if path == &PathBuf::from("-") {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).unwrap_or_default();
        input
    } else {
        match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                let message = format!("Failed to read {}: {}", path.display(), e);
                if !json {
                    println!("✗ {}", message);
                }
                let json_result = JsonResult {
                    path: path.display().to_string(),
                    success: false,
                    ast: None,
                    render: None,
                    errors: vec![JsonError {
                        line: 0,
                        col: 0,
                        message,
                    }],
                };
                return (false, 1, json_result);
            }
        }
    };

    let result = parse(&source);
    let success = result.errors.is_empty();
    let ast_value = if ast || json {
        serde_json::to_value(&result.file).ok()
    } else {
        None
    };

    let rendered = if render || json {
        Some(mimispec::render::render_file(&result.file))
    } else {
        None
    };

    let errors: Vec<JsonError> = result
        .errors
        .iter()
        .map(|err| JsonError {
            line: err.line(),
            col: err.col(),
            message: err.to_string(),
        })
        .collect();

    let json_result = JsonResult {
        path: path.display().to_string(),
        success,
        ast: ast_value,
        render: rendered,
        errors,
    };

    if json {
        (success, result.errors.len(), json_result)
    } else {
        if success {
            if render && !ast {
                if let Some(source) = &json_result.render {
                    print!("{}", source);
                }
            } else {
                println!("✓ Parsing successful: {}", path.display());
                if ast {
                    println!("{:#?}", result.file);
                }
                if render {
                    if let Some(source) = &json_result.render {
                        println!("{}", source);
                    }
                }
            }
        } else {
            eprintln!(
                "✗ Parsing failed for {} with {} error(s)",
                path.display(),
                result.errors.len()
            );
            for err in &result.errors {
                eprintln!("  - {:?}", err);
            }
        }
        (success, result.errors.len(), json_result)
    }
}

fn main() {
    let args = Args::parse();

    let mut total_errors = 0usize;
    let mut any_failure = false;
    let mut json_results = Vec::new();

    // 串行处理每个文件，避免并行占用过高
    for path in &args.files {
        let (ok, errs, json_result) = parse_one(path, args.ast, args.json, args.render);
        if !ok {
            any_failure = true;
        }
        total_errors += errs;
        json_results.push(json_result);
    }

    if args.json {
        let output = JsonOutput { results: json_results };
        println!("{}", serde_json::to_string_pretty(&output).unwrap_or_default());
    }

    if any_failure {
        if !args.json {
            eprintln!("\nTotal error(s): {}", total_errors);
        }
        std::process::exit(1);
    }
}
