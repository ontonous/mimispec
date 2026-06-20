use clap::Parser;
use mimispec::format::format_diagnostic;
use mimispec::latex::render_file_latex;
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

    /// Render math expressions as LaTeX (lightweight, for MathJax/KaTeX)
    #[arg(short, long)]
    latex: bool,
}

#[derive(Serialize)]
struct JsonError {
    code: String,
    line: usize,
    col: usize,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

#[derive(Serialize)]
struct JsonResult {
    path: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ast: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latex: Option<String>,
    errors: Vec<JsonError>,
}

#[derive(Serialize)]
struct JsonOutput {
    results: Vec<JsonResult>,
}

fn parse_one(
    path: &PathBuf,
    ast: bool,
    json: bool,
    render: bool,
    latex: bool,
) -> (bool, usize, JsonResult) {
    let source = if path == &PathBuf::from("-") {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .unwrap_or_default();
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
                    latex: None,
                    errors: vec![JsonError {
                        code: "E0000".into(),
                        line: 0,
                        col: 0,
                        message,
                        help: None,
                        suggestion: None,
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

    let latex_rendered = if latex || json {
        Some(render_file_latex(&result.file))
    } else {
        None
    };

    let errors: Vec<JsonError> = result
        .errors
        .iter()
        .map(|err| JsonError {
            code: err.code.to_string(),
            line: err.line,
            col: err.col,
            message: err.message.clone(),
            help: err.help.clone(),
            suggestion: err.suggestion.clone(),
        })
        .collect();

    let json_result = JsonResult {
        path: path.display().to_string(),
        success,
        ast: ast_value,
        render: rendered,
        latex: latex_rendered,
        errors,
    };

    if json {
        (success, result.errors.len(), json_result)
    } else {
        if success {
            if render && !ast && !latex {
                if let Some(source) = &json_result.render {
                    print!("{}", source);
                }
            } else if latex && !ast && !render {
                if let Some(source) = &json_result.latex {
                    println!("{}", source);
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
                if latex {
                    if let Some(source) = &json_result.latex {
                        println!("LaTeX:\n{}", source);
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
                eprintln!("{}", format_diagnostic(err, &source));
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
        let (ok, errs, json_result) = parse_one(path, args.ast, args.json, args.render, args.latex);
        if !ok {
            any_failure = true;
        }
        total_errors += errs;
        json_results.push(json_result);
    }

    if args.json {
        let output = JsonOutput {
            results: json_results,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
    }

    if any_failure {
        if !args.json {
            eprintln!("\nTotal error(s): {}", total_errors);
        }
        std::process::exit(1);
    }
}
