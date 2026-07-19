use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const USABILITY_SCHEMA_VERSION: &str = "mimispec.usability/0.3";

#[derive(Debug, Deserialize)]
struct TrialManifest {
    schema_version: String,
    status: String,
    authors: Vec<TrialAuthor>,
    documents: Vec<TrialDocument>,
    #[serde(default)]
    issues: Vec<TrialIssue>,
}

#[derive(Debug, Deserialize)]
struct TrialAuthor {
    id: String,
    independent: bool,
    first_valid_minutes: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct TrialDocument {
    id: String,
    author: String,
    domain: String,
    source: String,
}

#[derive(Debug, Deserialize)]
struct TrialIssue {
    id: String,
    severity: String,
    status: String,
    regression_fixture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsabilityReport {
    pub schema_version: &'static str,
    pub manifest: String,
    pub complete: bool,
    pub independent_authors: usize,
    pub documents: usize,
    pub domains: usize,
    pub five_minute_authors: usize,
    pub failures: Vec<String>,
}

pub fn check_trial_manifest(path: &Path) -> Result<UsabilityReport, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let manifest: TrialManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid {}: {error}", path.display()))?;
    if manifest.schema_version != USABILITY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported usability schema `{}`",
            manifest.schema_version
        ));
    }
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let independent = manifest
        .authors
        .iter()
        .filter(|author| author.independent)
        .map(|author| author.id.as_str())
        .collect::<HashSet<_>>();
    let five_minute = manifest
        .authors
        .iter()
        .filter(|author| {
            author.independent
                && author
                    .first_valid_minutes
                    .is_some_and(|minutes| minutes <= 5)
        })
        .count();
    let domains = manifest
        .documents
        .iter()
        .map(|document| document.domain.trim())
        .filter(|domain| !domain.is_empty())
        .collect::<HashSet<_>>();
    let mut failures = Vec::new();
    if manifest.status != "complete" {
        failures.push("trial status is not complete".into());
    }
    if independent.len() < 5 {
        failures.push(format!(
            "requires 5 independent authors, found {}",
            independent.len()
        ));
    }
    if manifest.documents.len() < 25 {
        failures.push(format!(
            "requires 25 documents, found {}",
            manifest.documents.len()
        ));
    }
    if domains.len() < 5 {
        failures.push(format!("requires 5 domains, found {}", domains.len()));
    }
    if five_minute < 4 {
        failures.push(format!(
            "requires 4 authors to create a valid file within five minutes, found {five_minute}"
        ));
    }

    let known_authors = manifest
        .authors
        .iter()
        .map(|author| author.id.as_str())
        .collect::<HashSet<_>>();
    for document in &manifest.documents {
        if !known_authors.contains(document.author.as_str()) {
            failures.push(format!(
                "document {} references unknown author {}",
                document.id, document.author
            ));
            continue;
        }
        let source_path = root.join(&document.source);
        let source = match fs::read_to_string(&source_path) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{}: {error}", source_path.display()));
                continue;
            }
        };
        let semantic = crate::parse(&source);
        let lossless = crate::parse_lossless(&source);
        if !semantic.errors.is_empty() {
            failures.push(format!(
                "document {} is partial: {:?}",
                document.id, semantic.errors
            ));
            continue;
        }
        if lossless.document.render_lossless() != source {
            failures.push(format!("document {} is not lossless-exact", document.id));
        }
        let rendered = crate::render::render_file(&semantic.file);
        let reparsed = crate::parse(&rendered);
        if !reparsed.errors.is_empty() || reparsed.file != semantic.file {
            failures.push(format!(
                "document {} failed semantic round-trip",
                document.id
            ));
        }
    }

    for issue in &manifest.issues {
        let blocking = matches!(issue.severity.to_ascii_lowercase().as_str(), "p0" | "p1")
            && issue.status != "closed";
        if blocking {
            failures.push(format!(
                "blocking issue {} remains {}",
                issue.id, issue.status
            ));
        }
        if issue.status == "closed" {
            match issue.regression_fixture.as_deref() {
                Some(fixture) if root.join(fixture).exists() => {}
                _ => failures.push(format!(
                    "closed issue {} lacks an existing regression fixture",
                    issue.id
                )),
            }
        }
    }

    Ok(UsabilityReport {
        schema_version: USABILITY_SCHEMA_VERSION,
        manifest: path.display().to_string(),
        complete: failures.is_empty(),
        independent_authors: independent.len(),
        documents: manifest.documents.len(),
        domains: domains.len(),
        five_minute_authors: five_minute,
        failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn release_trial_manifest_is_explicitly_incomplete_until_external_trial() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/usability/0.3/trial-manifest.json");
        let report = check_trial_manifest(&path).expect("trial manifest must load");
        assert!(!report.complete);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("5 independent authors")));
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("25 documents")));
    }
}
