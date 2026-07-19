use std::time::Instant;

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

        println!(
            "n={n} bytes={bytes} parse={parse_us}us render={render_us}us reparse={reparse_us}us lossless={lossless_us}us slots={slots}"
        );
    }
}
