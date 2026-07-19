use std::time::Instant;

use mimispec::session::{DocumentSession, SessionTextEdit, TextPosition, TextRange};

fn build_large(items: usize) -> String {
    let mut s = String::from("module Stress:\n");
    for i in 0..items {
        s.push_str(&format!(
            "    func F{i}(x, y):\n        requires: x > 0 and y > 0\n        ensures: x + y > 0\n        steps:\n            compute z\n            return z >>> done\n"
        ));
    }
    s
}

fn main() {
    // Warm
    let _ = mimispec::parse(&build_large(100));

    for &n in &[500, 1000, 2000] {
        let src = build_large(n);
        let bytes = src.len();

        let t = Instant::now();
        let r = mimispec::parse(&src);
        let parse_us = t.elapsed().as_micros();

        let t = Instant::now();
        let out = mimispec::render::render_file(&r.file);
        let render_us = t.elapsed().as_micros();

        let t = Instant::now();
        let _ = mimispec::parse(&out);
        let reparse_us = t.elapsed().as_micros();

        let t = Instant::now();
        let lr = mimispec::parse_lossless(&src);
        let lossless_us = t.elapsed().as_micros();

        let slots = mimispec::collaboration::collect_semantic_slot_snapshots(&lr.document).len();
        let t = Instant::now();
        let snapshot = mimispec::ide::ide_snapshot(&lr.document, &lr.errors);
        let snapshot_us = t.elapsed().as_micros();

        println!(
            "n={n} bytes={bytes} parse={parse_us}us render={render_us}us reparse={reparse_us}us lossless={lossless_us}us snapshot={snapshot_us}us slots={slots} queue_scopes={}",
            count_queue_scopes(&snapshot.diagnostics.queue_tree.root)
        );
    }

    for (name, source) in [
        (
            "mimi-kv",
            include_str!("../docs/corpora/mimi-kv-real-project.mms"),
        ),
        (
            "mimichat",
            include_str!("../docs/corpora/mimichat-real-project.mms"),
        ),
        (
            "mimi-markdown",
            include_str!("../docs/corpora/mimi-markdown-real-project.mms"),
        ),
        (
            "mimi-log",
            include_str!("../docs/corpora/mimi-log-real-project.mms"),
        ),
    ] {
        let started = Instant::now();
        let parsed = mimispec::parse_lossless(source);
        let parse_us = started.elapsed().as_micros();
        let started = Instant::now();
        let snapshot = mimispec::ide::ide_snapshot(&parsed.document, &parsed.errors);
        let snapshot_us = started.elapsed().as_micros();
        println!(
            "real={name} bytes={} parse={parse_us}us snapshot={snapshot_us}us slots={} decisions={} delegations={} queue_scopes={}",
            source.len(),
            snapshot.diagnostics.summary.total_slots,
            snapshot.diagnostics.decision_queue.len(),
            snapshot.diagnostics.delegation_queue.len(),
            count_queue_scopes(&snapshot.diagnostics.queue_tree.root),
        );
    }

    let mut session = DocumentSession::open("mem://utf16-perf.mms", "desc \"甲\"\n");
    let edits = (0..200)
        .map(|index| SessionTextEdit {
            range: Some(TextRange {
                start: TextPosition {
                    line: 0,
                    character: 6,
                },
                end: TextPosition {
                    line: 0,
                    character: 7,
                },
            }),
            text: if index % 2 == 0 { "乙" } else { "甲" }.into(),
        })
        .collect::<Vec<_>>();
    let started = Instant::now();
    session
        .observe_edits(&edits)
        .expect("valid sequential UTF-16 edits");
    println!(
        "utf16_edits={} elapsed={}us final_bytes={}",
        edits.len(),
        started.elapsed().as_micros(),
        session.source().len()
    );
}

fn count_queue_scopes(scope: &mimispec::diagnostics::QueueScopeNode) -> usize {
    1 + scope.children.iter().map(count_queue_scopes).sum::<usize>()
}
