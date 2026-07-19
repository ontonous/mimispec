use clap::{Parser, Subcommand};
use mimispec::format::format_diagnostic;
use mimispec::latex::render_file_latex;
use mimispec::parse;
use mimispec::resolver::Resolver;
use mimispec::symbol::SymbolTable;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const PARSE_JSON_SCHEMA_VERSION: &str = "mimispec.parse/0.3";

#[derive(Parser, Debug)]
#[command(
    name = "mimispec",
    version,
    about = "MimiSpec parser CLI",
    args_conflicts_with_subcommands = true
)]
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

    /// Include intent diagnostics (decision/delegation queues, attachment, gaps)
    #[arg(long, global = true)]
    diagnostics: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the long-lived MimiSpec 0.3 language server
    Lsp {
        /// Use Language Server Protocol framing over stdin/stdout
        #[arg(long, default_value_t = true)]
        stdio: bool,
    },
    /// Verify a MimiSpec 0.3 language-neutral conformance suite
    Conformance {
        #[command(subcommand)]
        command: ConformanceCommand,
    },
    /// Audit the independent 0.3 usability-release gate
    Usability {
        #[command(subcommand)]
        command: UsabilityCommand,
    },
    /// Check experimental Core-external provenance sidecars
    Provenance {
        #[command(subcommand)]
        command: ProvenanceCommand,
    },
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
    /// Analyze intent diagnostics for .mms file(s)
    Diagnose {
        /// .mms file(s) to analyze; use - for stdin
        #[arg(default_value = "-")]
        files: Vec<PathBuf>,
        /// Preserve the 0.3 compatibility view with ungrouped queue items
        #[arg(long)]
        flat_queues: bool,
    },
    /// Plan materialization from commit-ready locked slots
    Materialize {
        /// .mms file(s) to plan; use - for stdin
        #[arg(default_value = "-")]
        files: Vec<PathBuf>,
        /// Release scope label stored on the plan
        #[arg(long, default_value = "default")]
        scope: String,
    },
    /// Analyze a target profile against commit-ready intent
    Profile {
        /// .mms file(s) to analyze; use - for stdin
        #[arg(default_value = "-")]
        files: Vec<PathBuf>,
        /// Profile name: mimi (default), generic, rust, or typescript
        #[arg(long, default_value = "mimi")]
        target: String,
        /// Release scope label
        #[arg(long, default_value = "default")]
        scope: String,
    },
    /// Build an OSE-facing workflow board from slot states
    Workflow {
        /// .mms file(s) to analyze; use - for stdin
        #[arg(default_value = "-")]
        files: Vec<PathBuf>,
        /// Release scope label
        #[arg(long, default_value = "default")]
        scope: String,
    },
}

#[derive(Subcommand, Debug)]
enum ConformanceCommand {
    /// Check all parse, transition, lossless, and LSP fixtures in a manifest
    Check {
        /// Path to a mimispec.conformance/0.3 manifest
        #[arg(default_value = "tests/conformance/0.3/manifest.json")]
        manifest: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum UsabilityCommand {
    /// Check author, document, domain, round-trip, and issue requirements
    Check {
        #[arg(default_value = "tests/usability/0.3/trial-manifest.json")]
        manifest: PathBuf,
        /// Exit unsuccessfully unless every RC usability requirement is met
        #[arg(long)]
        require_complete: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ProvenanceCommand {
    /// Validate hashes, safe paths, exact slot locators, and drift
    Check {
        /// Path to a mimispec.provenance/0.1 JSON sidecar
        manifest: PathBuf,
        /// Explicit root containing every MMS/source path referenced by the sidecar
        #[arg(long)]
        source_root: PathBuf,
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
    partial: bool,
    status: mimispec::error::ParseStatus,
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
    schema_version: &'static str,
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
        success: !result.is_partial(),
        partial: result.is_partial(),
        status: result.status,
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
        if json_result.render.is_some() && json_result.ast.is_none() && json_result.latex.is_none()
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
                partial: true,
                status: mimispec::error::ParseStatus::Partial,
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
    let json_result = build_json_result(path, &result, ast || json, render || json, latex || json);

    if !json {
        print_cli_output(path, &result, &json_result, &source);
    }

    (json_result.success, result.errors.len(), json_result)
}

fn run_parse(files: &[PathBuf], ast: bool, json: bool, render: bool, latex: bool) {
    let mut total_errors = 0usize;
    let mut any_failure = false;
    let mut json_results = Vec::new();

    for path in files {
        let (ok, errs, json_result) = parse_one(path, ast, json, render, latex);
        if !ok {
            any_failure = true;
        }
        total_errors += errs;
        json_results.push(json_result);
    }

    if json {
        let output = JsonOutput {
            schema_version: PARSE_JSON_SCHEMA_VERSION,
            results: json_results,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                format!("{{\"error\": \"JSON serialization failed: {}\"}}", e)
            })
        );
    }

    if any_failure {
        if !json {
            eprintln!("\nTotal error(s): {}", total_errors);
        }
        std::process::exit(1);
    }
}

fn run_diagnose(files: &[PathBuf], json: bool, flat_queues: bool) {
    let mut reports = Vec::new();
    let mut any_syntax_error = false;

    for path in files {
        let source = match read_source(path) {
            Some(s) => s,
            None => {
                any_syntax_error = true;
                if !json {
                    eprintln!("{}: failed to read source", path.display());
                }
                continue;
            }
        };
        let result = mimispec::parse_lossless(&source);
        if !result.errors.is_empty() {
            any_syntax_error = true;
        }
        let report = mimispec::diagnostics::analyze_document(&result.document, &result.errors);
        if json {
            reports.push(serde_json::json!({
                "path": path.display().to_string(),
                "report": report,
            }));
        } else {
            print_diagnose_report(path, &report, flat_queues);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "results": reports }))
                .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
        );
    }

    if any_syntax_error {
        std::process::exit(1);
    }
}

fn run_materialize(files: &[PathBuf], scope: &str, json: bool) {
    let mut plans = Vec::new();
    let mut any_error = false;

    for path in files {
        let source = match read_source(path) {
            Some(s) => s,
            None => {
                any_error = true;
                if !json {
                    eprintln!("{}: failed to read source", path.display());
                }
                continue;
            }
        };
        let result = mimispec::parse_lossless(&source);
        if !result.errors.is_empty() {
            any_error = true;
        }
        let plan = mimispec::materialize::plan_materialization(&result.document, scope);
        if let Err(err) = mimispec::materialize::validate_plan(&plan) {
            any_error = true;
            if !json {
                eprintln!("{}: invalid plan: {err}", path.display());
            }
        }
        if json {
            plans.push(serde_json::json!({
                "path": path.display().to_string(),
                "plan": plan,
            }));
        } else {
            print_materialize_plan(path, &plan);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "results": plans }))
                .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
        );
    }

    if any_error {
        std::process::exit(1);
    }
}

fn run_provenance_check(manifest: &Path, source_root: &Path, json: bool) {
    match mimispec::provenance::check_manifest_path(manifest, source_root) {
        Ok(report) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_else(|error| {
                        format!("{{\"error\":\"JSON serialization failed: {error}\"}}")
                    })
                );
            } else {
                println!(
                    "provenance: valid={} checked_links={} schema={}",
                    report.valid, report.checked_links, report.schema_version
                );
                for finding in &report.findings {
                    println!(
                        "  - {}{}: {}",
                        finding.code,
                        finding
                            .link_index
                            .map(|index| format!(" [link {index}]"))
                            .unwrap_or_default(),
                        finding.message
                    );
                }
            }
            if !report.valid {
                std::process::exit(1);
            }
        }
        Err(error) => {
            if json {
                println!("{}", serde_json::json!({ "error": error }));
            } else {
                eprintln!("Provenance check failed: {error}");
            }
            std::process::exit(1);
        }
    }
}

fn run_profile(files: &[PathBuf], target: &str, scope: &str, json: bool) {
    let mut results = Vec::new();
    let mut any_error = false;

    for path in files {
        let source = match read_source(path) {
            Some(s) => s,
            None => {
                any_error = true;
                if !json {
                    eprintln!("{}: failed to read source", path.display());
                }
                continue;
            }
        };
        let parsed = mimispec::parse_lossless(&source);
        if !parsed.errors.is_empty() {
            any_error = true;
        }
        let analysis = match mimispec::profile::builtin_profile(target) {
            Some(profile) => profile.analyze(&parsed.document, scope),
            None => {
                any_error = true;
                if !json {
                    eprintln!(
                        "{}: unknown profile `{target}` (use mimi|generic|rust|typescript)",
                        path.display()
                    );
                }
                continue;
            }
        };
        if analysis
            .gaps
            .iter()
            .any(|gap| gap.severity == mimispec::profile::GapSeverity::Error)
        {
            any_error = true;
        }
        if json {
            results.push(serde_json::json!({
                "path": path.display().to_string(),
                "analysis": analysis,
            }));
        } else {
            print_profile_analysis(path, &analysis);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "results": results }))
                .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
        );
    }

    if any_error {
        std::process::exit(1);
    }
}

fn run_workflow(files: &[PathBuf], scope: &str, json: bool) {
    let mut results = Vec::new();
    let mut any_error = false;

    for path in files {
        let source = match read_source(path) {
            Some(s) => s,
            None => {
                any_error = true;
                if !json {
                    eprintln!("{}: failed to read source", path.display());
                }
                continue;
            }
        };
        let parsed = mimispec::parse_lossless(&source);
        if !parsed.errors.is_empty() {
            any_error = true;
        }
        let board = mimispec::workflow::build_workflow_board(&parsed.document, scope, &[]);
        if json {
            results.push(serde_json::json!({
                "path": path.display().to_string(),
                "board": board,
            }));
        } else {
            print_workflow_board(path, &board);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "results": results }))
                .unwrap_or_else(|e| format!("{{\"error\": \"JSON serialization failed: {}\"}}", e))
        );
    }

    if any_error {
        std::process::exit(1);
    }
}

fn print_workflow_board(path: &Path, board: &mimispec::workflow::WorkflowBoard) {
    println!("== {} ==", path.display());
    println!(
        "scope: {}  ready={}  decisions={} delegations={} challenges={} materialize={}",
        board.release_scope,
        board.readiness.ready,
        board.decision.len(),
        board.delegation.len(),
        board.lock_challenges.len(),
        board.materialization.len()
    );
    println!("readiness: {}", board.readiness.summary);
    if !board.decision.is_empty() {
        println!("decision queue:");
        for task in &board.decision {
            println!("  - {}", task.title);
        }
    }
    if !board.delegation.is_empty() {
        println!("delegation queue:");
        for task in &board.delegation {
            println!("  - {}", task.title);
        }
    }
    if !board.materialization.is_empty() {
        println!("materialization:");
        for task in &board.materialization {
            println!("  - {}", task.title);
        }
    }
    println!();
}

fn print_profile_analysis(path: &Path, analysis: &mimispec::profile::ProfileAnalysis) {
    println!("== {} ==", path.display());
    println!(
        "profile: {}@{}  supported={} partial={} gaps={}",
        analysis.profile.name,
        analysis.profile.version,
        analysis.supported_slots.len(),
        analysis.partial_slots.len(),
        analysis.gaps.len()
    );
    if !analysis.supported_slots.is_empty() {
        println!("supported:");
        for slot in &analysis.supported_slots {
            println!("  - {} ({})", slot.header.trim(), slot.state);
        }
    }
    if !analysis.partial_slots.is_empty() {
        println!("partial:");
        for slot in &analysis.partial_slots {
            println!("  - {} ({})", slot.header.trim(), slot.state);
        }
    }
    if !analysis.gaps.is_empty() {
        println!("gaps:");
        for gap in &analysis.gaps {
            println!(
                "  - [{:?}] {} — {}",
                gap.severity,
                gap.slot_header.trim(),
                gap.reason
            );
            println!("      action: {}", gap.suggested_action);
        }
    }
    println!();
}

fn print_materialize_plan(path: &Path, plan: &mimispec::materialize::MaterializationPlan) {
    println!("== {} ==", path.display());
    println!("release scope: {}", plan.selection.release_scope);
    println!(
        "selected: {}  excluded unlocked: {}  evidence: {}",
        plan.selection.slots.len(),
        plan.excluded_unlocked.len(),
        plan.evidence.len()
    );
    if !plan.selection.slots.is_empty() {
        println!("commit-ready slots:");
        for slot in &plan.selection.slots {
            println!(
                "  - [{:?}] {} ({})",
                slot.provenance,
                slot.header.trim(),
                slot.state
            );
        }
    }
    if !plan.excluded_unlocked.is_empty() {
        println!("excluded unlocked:");
        for slot in &plan.excluded_unlocked {
            println!("  - {} ({})", slot.header.trim(), slot.state);
        }
    }
    println!();
}

fn print_diagnose_report(
    path: &Path,
    report: &mimispec::diagnostics::DocumentDiagnostics,
    flat_queues: bool,
) {
    println!("== {} ==", path.display());
    println!(
        "summary: slots={} commit_ready={} decision={} delegation={}",
        report.summary.total_slots,
        report.summary.commit_ready,
        report.decision_queue.len(),
        report.delegation_queue.len()
    );
    if flat_queues && !report.decision_queue.is_empty() {
        println!("decision queue:");
        for item in &report.decision_queue {
            println!("  - [{}] {}", item.state, item.header.trim());
        }
    }
    if flat_queues && !report.delegation_queue.is_empty() {
        println!("delegation queue:");
        for item in &report.delegation_queue {
            println!("  - [{}] {}", item.state, item.header.trim());
        }
    }
    if !flat_queues
        && (report.queue_tree.root.decision_count > 0
            || report.queue_tree.root.delegation_count > 0)
    {
        println!("queues by scope:");
        print_queue_scope(&report.queue_tree.root, 1);
    }
    if !report.diagnostics.is_empty() {
        println!("diagnostics:");
        for diagnostic in &report.diagnostics {
            println!(
                "  - {} {:?} {}",
                diagnostic.code.0, diagnostic.severity, diagnostic.message
            );
            if let Some(help) = &diagnostic.help {
                println!("      help: {help}");
            }
        }
    }
    println!();
}

fn print_queue_scope(scope: &mimispec::diagnostics::QueueScopeNode, depth: usize) {
    let indent = "  ".repeat(depth);
    println!(
        "{indent}{} [decision={} delegation={}]",
        scope.header, scope.decision_count, scope.delegation_count
    );
    for item in &scope.items {
        println!("{indent}  - [{}] {}", item.state, item.anchor);
    }
    for child in &scope.children {
        print_queue_scope(child, depth + 1);
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

    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let canonical = match current.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !visited.insert(canonical) {
            continue;
        }
        let entries = match fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    stack.push(path);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("mms") {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Lsp { stdio }) => {
            if !stdio {
                eprintln!("only --stdio transport is supported in MimiSpec 0.3");
                std::process::exit(2);
            }
            if let Err(error) = mimispec::lsp::run_stdio() {
                eprintln!("MimiSpec LSP failed: {error}");
                std::process::exit(1);
            }
        }
        Some(Commands::Conformance { command }) => match command {
            ConformanceCommand::Check { manifest } => {
                match mimispec::conformance::check_manifest(manifest) {
                    Ok(report) => {
                        if cli.json {
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&report)
                                    .unwrap_or_else(|e| format!("{{\"error\": \"conformance report serialization failed: {e}\"}}"))
                            );
                        } else {
                            println!(
                                "MimiSpec conformance: {}/{} passed ({})",
                                report.passed, report.total, report.manifest
                            );
                            for failure in &report.failures {
                                eprintln!("  {}: {}", failure.case, failure.message);
                            }
                        }
                        if !report.success() {
                            std::process::exit(1);
                        }
                    }
                    Err(error) => {
                        eprintln!("Conformance check failed: {error}");
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Usability { command }) => match command {
            UsabilityCommand::Check {
                manifest,
                require_complete,
            } => match mimispec::usability::check_trial_manifest(manifest) {
                Ok(report) => {
                    if cli.json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&report).unwrap_or_else(|e| format!(
                                "{{\"error\": \"usability report serialization failed: {e}\"}}"
                            ))
                        );
                    } else {
                        println!(
                            "MimiSpec usability: authors={} documents={} domains={} five-minute={} complete={}",
                            report.independent_authors,
                            report.documents,
                            report.domains,
                            report.five_minute_authors,
                            report.complete
                        );
                        for failure in &report.failures {
                            eprintln!("  {failure}");
                        }
                    }
                    if *require_complete && !report.complete {
                        std::process::exit(1);
                    }
                }
                Err(error) => {
                    eprintln!("Usability check failed: {error}");
                    std::process::exit(1);
                }
            },
        },
        Some(Commands::Provenance { command }) => match command {
            ProvenanceCommand::Check {
                manifest,
                source_root,
            } => run_provenance_check(manifest, source_root, cli.json),
        },
        Some(Commands::Parse { files }) => {
            run_parse(files, cli.ast, cli.json, cli.render, cli.latex);
        }
        Some(Commands::Build { dir }) => {
            run_build(dir);
        }
        Some(Commands::Diagnose { files, flat_queues }) => {
            run_diagnose(files, cli.json, *flat_queues);
        }
        Some(Commands::Materialize { files, scope }) => {
            run_materialize(files, scope, cli.json);
        }
        Some(Commands::Profile {
            files,
            target,
            scope,
        }) => {
            run_profile(files, target, scope, cli.json);
        }
        Some(Commands::Workflow { files, scope }) => {
            run_workflow(files, scope, cli.json);
        }
        None => {
            if cli.diagnostics {
                run_diagnose(&cli.files, cli.json, false);
            } else {
                // Flat args = parse behavior (backward compatible)
                run_parse(&cli.files, cli.ast, cli.json, cli.render, cli.latex);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_output_is_versioned_and_marks_partial_documents() {
        let _: serde_json::Value = serde_json::from_str(include_str!(
            "../docs/schemas/parse-output-v0.3.schema.json"
        ))
        .expect("parse-output schema must be valid JSON");
        let complete = mimispec::parse("desc \"ready\"\n");
        let complete = build_json_result(Path::new("complete.mms"), &complete, true, true, false);
        let partial = mimispec::parse("func Broken(:\n");
        let partial = build_json_result(Path::new("partial.mms"), &partial, true, false, false);
        let value = serde_json::to_value(JsonOutput {
            schema_version: PARSE_JSON_SCHEMA_VERSION,
            results: vec![complete, partial],
        })
        .expect("parse output must serialize");

        assert_eq!(value["schema_version"], PARSE_JSON_SCHEMA_VERSION);
        assert_eq!(value["results"][0]["status"], "complete");
        assert_eq!(value["results"][0]["partial"], false);
        assert_eq!(
            value["results"][0]["ast"]["schema_version"],
            mimispec::ast::AST_SCHEMA_VERSION
        );
        assert_eq!(value["results"][1]["status"], "partial");
        assert_eq!(value["results"][1]["partial"], true);
    }
}
