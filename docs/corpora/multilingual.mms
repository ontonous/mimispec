// MimiSpec 0.3 Core acceptance corpus — multilingual descriptions.
//
// Exercises the M5 invariant that natural language is first-class and
// that Unicode descriptions and rules preserve exact content across scripts.
// Pairs with `src/lib/mod.rs::multilingual_tests`, which asserts byte-exact
// round-trip.
//
// Part of the M5 corpus deliverable (roadmap §10).
//
// Scripts covered: Simplified Chinese, Traditional Chinese, Japanese,
// Korean, Arabic (RTL), Cyrillic, emoji + Latin.

desc$ "一个意图可以同时被多种语言表达，parser 不得归一化或替换"
desc "同じ意図を複数の言語で表現できる"

rule "النوايا يجب أن تبقى واضحة عبر اللغات"
rule "모든 의도는 언어 간에 명확하게 유지되어야 한다"
rule "all payments must be idempotent 🔒💵"
rule "платежи должны быть идемпотентными"

desc$ "家庭记录日常开销：老人也可以轻松使用"
desc "家庭記錄日常開銷：老人也可以輕鬆使用"

module$ MultilingualBoundary:
    desc$ "多语言意图的边界"
    desc "多言語の意図の境界"

    func$ DescribeInLanguage(intent, language):
        desc$ "用指定语言表达意图"
        requires$: intent.kind_is_known == true
        requires$: language.supported == true
        ensures$: description.readable == true
        rule$ "翻译不得改变意图的锁定状态"

rule$ "語言切換不得改變意圖的鎖定狀態"
rule "언어 전환은 의도의 잠금 상태를 변경하지 않는다"
