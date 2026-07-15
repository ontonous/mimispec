use serde::Serialize;

use crate::ast::{Fragment, Step};
use crate::lossless::LosslessDocument;
use crate::materialize::{
    plan_materialization, CommitSelection, MaterializationPlan, MaterializationSlot, Provenance,
};

/// Capability matrix reported by a target profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetCapabilities {
    pub modules: bool,
    pub types: bool,
    pub functions: bool,
    pub flows: bool,
    pub ui: bool,
    pub contracts: bool,
    pub concurrency: bool,
    pub formal_verification: bool,
    pub notes: Vec<String>,
}

impl TargetCapabilities {
    pub fn mimi_native() -> Self {
        Self {
            modules: true,
            types: true,
            functions: true,
            flows: true,
            ui: false,
            contracts: true,
            concurrency: true,
            formal_verification: true,
            notes: vec![
                "Mimi is the first-party native target.".into(),
                "UI views require a separate frontend profile or adapter.".into(),
                "Formal verification depends on available Z3 tooling.".into(),
            ],
        }
    }

    pub fn generic_minimal() -> Self {
        Self {
            modules: true,
            types: true,
            functions: true,
            flows: false,
            ui: false,
            contracts: false,
            concurrency: false,
            formal_verification: false,
            notes: vec![
                "Minimal generic profile for languages without first-class Flow/contracts.".into(),
            ],
        }
    }

    pub fn rust_reference() -> Self {
        Self {
            modules: true,
            types: true,
            functions: true,
            flows: false,
            ui: false,
            contracts: true,
            concurrency: true,
            formal_verification: false,
            notes: vec![
                "Rust reference profile maps modules/types/functions deeply.".into(),
                "Flow intent is partial: encode as enums + match or a state crate.".into(),
                "Formal verification is out of band (not assumed).".into(),
            ],
        }
    }

    pub fn typescript_reference() -> Self {
        Self {
            modules: true,
            types: true,
            functions: true,
            flows: false,
            ui: true,
            contracts: false,
            concurrency: false,
            formal_verification: false,
            notes: vec![
                "TypeScript reference profile maps modules/types/functions and UI residuals."
                    .into(),
                "Contracts and Flow are partial/unsupported and must be reported.".into(),
            ],
        }
    }
}

/// Stable target profile protocol for Core materialization clients.
pub trait TargetProfile {
    fn id(&self) -> ProfileId;
    fn capabilities(&self) -> TargetCapabilities;
    fn analyze(&self, document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MimiProfile;

#[derive(Debug, Clone, Copy, Default)]
pub struct GenericProfile;

#[derive(Debug, Clone, Copy, Default)]
pub struct RustProfile;

#[derive(Debug, Clone, Copy, Default)]
pub struct TypeScriptProfile;

impl TargetProfile for MimiProfile {
    fn id(&self) -> ProfileId {
        ProfileId {
            name: "mimi".into(),
            version: "0.30.0+".into(),
        }
    }

    fn capabilities(&self) -> TargetCapabilities {
        TargetCapabilities::mimi_native()
    }

    fn analyze(&self, document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
        let plan = plan_materialization(document, release_scope);
        analyze_profile(document, self.id(), self.capabilities(), plan)
    }
}

impl TargetProfile for GenericProfile {
    fn id(&self) -> ProfileId {
        ProfileId {
            name: "generic".into(),
            version: "0.1.0".into(),
        }
    }

    fn capabilities(&self) -> TargetCapabilities {
        TargetCapabilities::generic_minimal()
    }

    fn analyze(&self, document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
        let plan = plan_materialization(document, release_scope);
        analyze_profile(document, self.id(), self.capabilities(), plan)
    }
}

impl TargetProfile for RustProfile {
    fn id(&self) -> ProfileId {
        ProfileId {
            name: "rust".into(),
            version: "0.1.0".into(),
        }
    }

    fn capabilities(&self) -> TargetCapabilities {
        TargetCapabilities::rust_reference()
    }

    fn analyze(&self, document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
        let plan = plan_materialization(document, release_scope);
        analyze_profile(document, self.id(), self.capabilities(), plan)
    }
}

impl TargetProfile for TypeScriptProfile {
    fn id(&self) -> ProfileId {
        ProfileId {
            name: "typescript".into(),
            version: "0.1.0".into(),
        }
    }

    fn capabilities(&self) -> TargetCapabilities {
        TargetCapabilities::typescript_reference()
    }

    fn analyze(&self, document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
        let plan = plan_materialization(document, release_scope);
        analyze_profile(document, self.id(), self.capabilities(), plan)
    }
}

/// Resolve a built-in profile by name.
pub fn builtin_profile(name: &str) -> Option<Box<dyn TargetProfile>> {
    match name {
        "mimi" => Some(Box::new(MimiProfile)),
        "generic" => Some(Box::new(GenericProfile)),
        "rust" => Some(Box::new(RustProfile)),
        "typescript" | "ts" => Some(Box::new(TypeScriptProfile)),
        _ => None,
    }
}

/// Conformance helper: profiles must report every selected slot as supported,
/// partial, or an explicit gap.
pub fn profile_conformance(
    profile: &dyn TargetProfile,
    document: &LosslessDocument,
) -> Result<(), String> {
    let analysis = profile.analyze(document, "conformance");
    assert_no_silent_drops(&analysis.plan.selection, &analysis)?;
    if analysis.capabilities.notes.is_empty() {
        return Err("profile must declare capability notes".into());
    }
    Ok(())
}

/// One intent that the selected profile cannot fully satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityGap {
    pub slot_header: String,
    pub reason: String,
    pub severity: GapSeverity,
    pub suggested_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapSeverity {
    Error,
    Warning,
    Info,
}

/// Versioned target profile identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileId {
    pub name: String,
    pub version: String,
}

/// Result of probing a target profile against a materialization plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileAnalysis {
    pub profile: ProfileId,
    pub capabilities: TargetCapabilities,
    pub plan: MaterializationPlan,
    pub gaps: Vec<CapabilityGap>,
    pub supported_slots: Vec<MaterializationSlot>,
    pub partial_slots: Vec<MaterializationSlot>,
}

/// Probe the first-party Mimi profile without requiring MIMI to be installed.
///
/// This reports mapping readiness and capability gaps. Actual `.mimi` generation
/// remains an external adapter responsibility.
pub fn analyze_mimi_profile(document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
    MimiProfile.analyze(document, release_scope)
}

/// Probe a generic minimal profile for non-Mimi targets.
pub fn analyze_generic_profile(
    document: &LosslessDocument,
    release_scope: &str,
) -> ProfileAnalysis {
    GenericProfile.analyze(document, release_scope)
}

pub fn analyze_rust_profile(document: &LosslessDocument, release_scope: &str) -> ProfileAnalysis {
    RustProfile.analyze(document, release_scope)
}

pub fn analyze_typescript_profile(
    document: &LosslessDocument,
    release_scope: &str,
) -> ProfileAnalysis {
    TypeScriptProfile.analyze(document, release_scope)
}

fn analyze_profile(
    document: &LosslessDocument,
    profile: ProfileId,
    capabilities: TargetCapabilities,
    plan: MaterializationPlan,
) -> ProfileAnalysis {
    let mut gaps = Vec::new();
    let mut supported_slots = Vec::new();
    let mut partial_slots = Vec::new();

    for slot in &plan.selection.slots {
        match classify_slot(document, slot, &capabilities) {
            SlotSupport::Supported => supported_slots.push(slot.clone()),
            SlotSupport::Partial(reason) => {
                partial_slots.push(annotate_partial(slot.clone()));
                gaps.push(CapabilityGap {
                    slot_header: slot.header.clone(),
                    reason,
                    severity: GapSeverity::Warning,
                    suggested_action:
                        "Materialize as partial intent and record the gap as evidence.".into(),
                });
            }
            SlotSupport::Unsupported(reason) => {
                gaps.push(CapabilityGap {
                    slot_header: slot.header.clone(),
                    reason,
                    severity: GapSeverity::Error,
                    suggested_action:
                        "Do not silently drop this intent; keep it unresolved or choose another profile."
                            .into(),
                });
            }
        }
    }

    // Unlocked excluded slots are not errors; surface them as info gaps for planning.
    for slot in &plan.excluded_unlocked {
        gaps.push(CapabilityGap {
            slot_header: slot.header.clone(),
            reason: "Slot is not commit-ready and was excluded from confirmed materialization."
                .into(),
            severity: GapSeverity::Info,
            suggested_action: "Lock the slot after review if it belongs in this release scope."
                .into(),
        });
    }

    // Document-level structural gaps independent of selection.
    gaps.extend(document_level_gaps(document, &capabilities));

    ProfileAnalysis {
        profile,
        capabilities,
        plan,
        gaps,
        supported_slots,
        partial_slots,
    }
}

enum SlotSupport {
    Supported,
    Partial(String),
    Unsupported(String),
}

fn classify_slot(
    document: &LosslessDocument,
    slot: &MaterializationSlot,
    capabilities: &TargetCapabilities,
) -> SlotSupport {
    use crate::lossless::SourceNodeKind;
    match slot.kind {
        SourceNodeKind::Module if capabilities.modules => SlotSupport::Supported,
        SourceNodeKind::Module => {
            SlotSupport::Unsupported("Profile does not support modules.".into())
        }
        SourceNodeKind::TypeDef if capabilities.types => SlotSupport::Supported,
        SourceNodeKind::TypeDef => {
            SlotSupport::Unsupported("Profile does not support types.".into())
        }
        SourceNodeKind::Func if capabilities.functions => {
            if !capabilities.contracts && func_has_contracts(document, &slot.header) {
                SlotSupport::Partial(
                    "Function contracts are present but formal contracts are unsupported.".into(),
                )
            } else {
                SlotSupport::Supported
            }
        }
        SourceNodeKind::Func => {
            SlotSupport::Unsupported("Profile does not support functions.".into())
        }
        SourceNodeKind::Flow if capabilities.flows => SlotSupport::Supported,
        SourceNodeKind::Flow => {
            SlotSupport::Unsupported("Profile does not support first-class flows.".into())
        }
        SourceNodeKind::Ui if capabilities.ui => SlotSupport::Supported,
        SourceNodeKind::Ui => SlotSupport::Partial(
            "UI fragment is outside this profile's deep support; export as residual intent.".into(),
        ),
        SourceNodeKind::Steps | SourceNodeKind::Expr | SourceNodeKind::UiNode
            if capabilities.functions =>
        {
            SlotSupport::Partial("Standalone fragment will need target-specific packaging.".into())
        }
        _ => {
            SlotSupport::Partial("Nested or placeholder slot needs target-specific mapping.".into())
        }
    }
}

fn annotate_partial(mut slot: MaterializationSlot) -> MaterializationSlot {
    slot.provenance = Provenance::TargetDerived;
    slot
}

fn func_has_contracts(document: &LosslessDocument, header: &str) -> bool {
    document.semantic().fragments.iter().any(|fragment| {
        if let Fragment::Func { func } = fragment {
            header.contains(&func.name.name)
                && (func.requires.is_some() || func.ensures.is_some() || !func.rules.is_empty())
        } else {
            false
        }
    })
}

fn document_level_gaps(
    document: &LosslessDocument,
    capabilities: &TargetCapabilities,
) -> Vec<CapabilityGap> {
    let mut gaps = Vec::new();
    for fragment in &document.semantic().fragments {
        match fragment {
            Fragment::Flow { flow } if !capabilities.flows => {
                gaps.push(CapabilityGap {
                    slot_header: format!("flow {}", flow.name.name),
                    reason: "Document contains Flow intent unsupported by the selected profile."
                        .into(),
                    severity: GapSeverity::Error,
                    suggested_action:
                        "Choose a Flow-capable profile such as Mimi, or rewrite as steps.".into(),
                });
            }
            Fragment::Func { func } if !capabilities.formal_verification && func.math.is_some() => {
                gaps.push(CapabilityGap {
                    slot_header: format!("func {}", func.name.name),
                    reason: "Math/contract blocks cannot be formally verified by this profile."
                        .into(),
                    severity: GapSeverity::Warning,
                    suggested_action: "Keep math as documentation or select a verifying profile."
                        .into(),
                });
            }
            Fragment::Func { func }
                if capabilities.concurrency && steps_need_concurrency(&func.steps) => {}
            Fragment::Ui { ui } if !capabilities.ui => {
                gaps.push(CapabilityGap {
                    slot_header: format!("ui {}", ui.name.name),
                    reason: "UI intent is outside the selected profile's native capabilities."
                        .into(),
                    severity: GapSeverity::Warning,
                    suggested_action: "Route UI fragments to a frontend profile.".into(),
                });
            }
            _ => {}
        }
    }
    gaps
}

fn steps_need_concurrency(steps: &[Step]) -> bool {
    steps
        .iter()
        .any(|step| matches!(step, Step::Parasteps { .. }))
}

/// Ensure unsupported intent is reported rather than dropped from a selection.
pub fn assert_no_silent_drops(
    selection: &CommitSelection,
    analysis: &ProfileAnalysis,
) -> Result<(), String> {
    for slot in &selection.slots {
        let mentioned = analysis
            .supported_slots
            .iter()
            .chain(analysis.partial_slots.iter())
            .any(|reported| reported.node == slot.node)
            || analysis
                .gaps
                .iter()
                .any(|gap| gap.slot_header == slot.header);
        if !mentioned {
            return Err(format!(
                "profile silently ignored slot `{}`",
                slot.header.trim()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_lossless;

    #[test]
    fn mimi_profile_supports_locked_func_and_flow() {
        let source = r#"func Pay$:
    steps:
        charge payment

flow$ Checkout:
    Pending >>> Paid:
"#;
        let doc = parse_lossless(source).document;
        let analysis = analyze_mimi_profile(&doc, "payments");
        assert_eq!(analysis.profile.name, "mimi");
        assert!(analysis.capabilities.flows);
        assert!(analysis
            .supported_slots
            .iter()
            .any(|slot| slot.header.contains("Pay")));
        assert!(analysis
            .supported_slots
            .iter()
            .any(|slot| slot.header.contains("Checkout")));
        assert!(assert_no_silent_drops(&analysis.plan.selection, &analysis).is_ok());
    }

    #[test]
    fn generic_profile_reports_flow_gap() {
        let source = r#"flow$ Checkout:
    Pending >>> Paid:

func Pay$:
    steps:
        charge payment
"#;
        let doc = parse_lossless(source).document;
        let analysis = analyze_generic_profile(&doc, "payments");
        assert_eq!(analysis.profile.name, "generic");
        assert!(!analysis.capabilities.flows);
        assert!(analysis.gaps.iter().any(|gap| {
            gap.severity == GapSeverity::Error && gap.reason.contains("flows")
                || gap.slot_header.contains("Checkout")
        }));
    }

    #[test]
    fn unlocked_slots_are_info_gaps_not_confirmed() {
        let source = r#"func Draft??:
    steps:
        todo

func Pay$:
    steps:
        charge payment
"#;
        let doc = parse_lossless(source).document;
        let analysis = analyze_mimi_profile(&doc, "core");
        assert!(analysis
            .plan
            .selection
            .slots
            .iter()
            .all(|slot| slot.commit_ready));
        assert!(analysis
            .gaps
            .iter()
            .any(|gap| { gap.severity == GapSeverity::Info && gap.slot_header.contains("Draft") }));
    }

    #[test]
    fn target_profile_trait_and_conformance_cover_builtins() {
        let source = r#"func Pay$:
    requires: true
    steps:
        charge payment

flow$ Checkout:
    Pending >>> Paid:

ui Panel$:
    stack:
        "Title"
"#;
        let doc = parse_lossless(source).document;
        for name in ["mimi", "generic", "rust", "typescript"] {
            let profile = builtin_profile(name).expect(name);
            profile_conformance(profile.as_ref(), &doc).expect(name);
        }

        let rust = analyze_rust_profile(&doc, "pay");
        assert!(rust
            .gaps
            .iter()
            .any(|gap| gap.slot_header.contains("Checkout")));

        let ts = analyze_typescript_profile(&doc, "pay");
        assert!(ts.capabilities.ui);
        assert!(
            ts.supported_slots
                .iter()
                .chain(ts.partial_slots.iter())
                .any(|slot| slot.header.contains("Panel")
                    || slot.kind == crate::lossless::SourceNodeKind::Ui)
                || ts.gaps.iter().any(|gap| gap.slot_header.contains("Panel"))
        );
    }
}
