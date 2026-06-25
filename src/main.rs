use clap::{Parser, Subcommand};
use mimispec::format::format_diagnostic;
use mimispec::latex::render_file_latex;
use mimispec::parse;
use mimispec::resolver::Resolver;
use mimispec::symbol::SymbolTable;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "mimispec", version, about = "MimiSpec parser CLI", args_conflicts_with_subcommands = true)]
struct Cli {
    /// .mms file(s) to parse; use - for stdin
    #[arg(default_value = "-")]
    files: Vec<PathBuf>,

    /// Show AST structure
    #[arg(short, long, global = true)]
    ast: bool,

    /// Output results as JSON (useful for editor integrations)
    #[arg(short, long, global = true)]
    json: bool,

    /// Render AST back to MimiSpec source
    #[arg(short, long, global = true)]
    render: bool,

    /// Render math expressions as LaTeX (lightweight, for MathJax/KaTeX)
    #[arg(short, long, global = true)]
    latex: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Parse .mms file(s) and show diagnostics
    Parse {
        /// .mms file(s) to parse; use - for stdin
        #[arg(default_value = "-")]
        files: Vec<PathBuf>,
    },
    /// Build a directory of .mms files with cross-file resolution
    Build {
        /// Root directory to scan for .mms files
        dir: PathBuf,
    },
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

fn read_source(path: &Path) -> Option<String> {
    if path == Path::new("-") {
        use std::io::Read;
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).ok()?;
        Some(input)
    } else {
        fs::read_to_string(path).ok()
    }
}

fn build_json_result(
    path: &Path,
    result: &mimispec::error::ParseResult,
    ast: bool,
    render: bool,
    latex: bool,
) -> JsonResult {
    let ast_value = if ast {
        serde_json::to_value(&result.file).ok()
    } else {
        None
    };
    let rendered = if render {
        Some(mimispec::render::render_file(&result.file))
    } else {
        None
    };
    let latex_rendered = if latex {
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
    JsonResult {
        path: path.display().to_string(),
        success: result.errors.is_empty(),
        ast: ast_value,
        render: rendered,
        latex: latex_rendered,
        errors,
    }
}

fn print_cli_output(
    path: &Path,
    result: &mimispec::error::ParseResult,
    json_result: &JsonResult,
    source: &str,
) {
    if result.errors.is_empty() {
        if json_result.render.is_some()
            && json_result.ast.is_none()
            && json_result.latex.is_none()
        {
            if let Some(source) = &json_result.render {
                print!("{}", source);
            }
        } else if json_result.latex.is_some()
            && json_result.ast.is_none()
            && json_result.render.is_none()
        {
            if let Some(source) = &json_result.latex {
                println!("{}", source);
            }
        } else {
            println!("✓ Parsing successful: {}", path.display());
            if json_result.ast.is_some() {
                println!("{:#?}", result.file);
            }
            if let Some(source) = &json_result.render {
                println!("{}", source);
            }
            if let Some(source) = &json_result.latex {
                println!("LaTeX:\n{}", source);
            }
        }
    } else {
        eprintln!(
            "✗ Parsing failed for {} with {} error(s)",
            path.display(),
            result.errors.len()
        );
        for err in &result.errors {
            eprintln!("{}", format_diagnostic(err, source));
        }
    }
}

fn parse_one(
    path: &Path,
    ast: bool,
    json: bool,
    render: bool,
    latex: bool,
) -> (bool, usize, JsonResult) {
    let source = match read_source(path) {
        Some(s) => s,
        None => {
            let message = format!("Failed to read {}", path.display());
            if !json {
                eprintln!("✗ {}", message);
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
    };

    let result = parse(&source);
    let json_result =
        build_json_result(path, &result, ast || json, render || json, latex || json);

    if !json {
        print_cli_output(path, &result, &json_result, &source);
    }

    (json_result.success, result.errors.len(), json_result)
}

fn run_parse(
    files: &[PathBuf],
    ast: bool,
    json: bool,
    render: bool,
    latex: bool,
) {
    let mut total_errors = 0usize;
    let mut any_failure = false;
    let mut json_results = Vec::new();

    for path in files {
        let (ok, errs, json_result) =
            parse_one(path, ast, json, render, latex);
        if !ok {
            any_failure = true;
        }
        total_errors += errs;
        json_results.push(json_result);
    }

    if json {
        let output = JsonOutput {
            results: json_results,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_default()
        );
    }

    if any_failure {
        if !json {
            eprintln!("\nTotal error(s): {}", total_errors);
        }
        std::process::exit(1);
    }
}

fn run_build(dir: &Path) {
    let mut total_errors = 0usize;
    let mut any_failure = false;

    // Walk directory for .mms files
    let mms_files = match find_mms_files(dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("Error scanning directory: {}", e);
            std::process::exit(1);
        }
    };

    if mms_files.is_empty() {
        eprintln!("No .mms files found in {}", dir.display());
        std::process::exit(0);
    }

    let mut resolver = Resolver::new(dir.to_path_buf());

    for path in &mms_files {
        resolver.resolve(path);
    }

    let resolve_errors = resolver.take_errors();
    let files = resolver.take_files();

    // Report resolve errors
    for (_path, _err) in &resolve_errors {
        any_failure = true;
        total_errors += 1;
    }

    // Build symbol table
    let symbols = SymbolTable::build(&files);
    if symbols.has_conflicts() {
        println!("\n⚠ Name conflicts detected:");
        for conflict in symbols.conflicts() {
            println!(
                "  '{}' defined in {} locations:",
                conflict.name,
                conflict.entries.len()
            );
            for entry in &conflict.entries {
                println!("    - {} ({})", entry.file.display(), entry.kind);
            }
        }
        any_failure = true;
    }

    // Output summary
    let success_count = mms_files.len() - resolve_errors.len();
    println!(
        "✓ Build complete: {} file(s) parsed, {} error(s)",
        success_count, total_errors
    );

    if !symbols.all_names().is_empty() {
        println!("\nDefined names ({}):", symbols.all_names().len());
        for name in symbols.all_names() {
            let entries = symbols.lookup(name);
            let kinds: Vec<String> = entries.iter().map(|e| e.kind.to_string()).collect();
            println!("  {} [{}]", name, kinds.join(", "));
        }
    }

    if any_failure {
        println!("\nResolve errors:");
        for (path, err) in &resolve_errors {
            println!("  {}: {}", path.display(), err);
        }
        std::process::exit(1);
    }
}

fn find_mms_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut visited = std::collections::HashSet::new();
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }

    walk_dir(dir, &mut files, &mut visited).map_err(|e| e.to_string())?;
    files.sort();
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
) -> std::io::Result<()> {
    let canonical = match dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };
    if !visited.insert(canonical) {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories and common non-source dirs
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                walk_dir(&path, files, visited)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("mms") {
            files.push(path);
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Parse { files }) => {
            run_parse(files, cli.ast, cli.json, cli.render, cli.latex);
        }
        Some(Commands::Build { dir }) => {
            run_build(dir);
        }
        None => {
            // Flat args = parse behavior (backward compatible)
            run_parse(&cli.files, cli.ast, cli.json, cli.render, cli.latex);
        }
    }
}
