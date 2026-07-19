// MimiSpec 0.3 Core acceptance corpus — plain-language product intent.
//
// Exercises the five-minute entry point: a non-programmer describes what they
// want, what must not be violated, and what they delegate to AI. No named
// wrapper, no type system, no target-language knowledge required.
//
// This file is part of the M5 corpus deliverable (roadmap §10). It must
// parse cleanly under the canonical parser and round-trip through the
// semantic renderer without loss.

desc?? "我想做一个帮助家庭记录日常开销的应用"

rule "老人也可以轻松使用"
rule "财务数据默认只保存在本地"
rule "每次记录都要能离线完成"

desc$ "界面只问三件事：花了多少、花在哪、什么时候"
desc "预算超支时温和提醒，不阻止记录"

rule "分类可以由 AI 建议，但用户总能修改"
rule?? "数据同步策略交给 AI 评估"

requires: entry.amount > 0
requires: entry.timestamp_known == true
ensures: entry.persisted_locally == true
