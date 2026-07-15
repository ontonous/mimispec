use serde::Serialize;

use crate::ast::{Commitment, LockIntent};

/// 请求实际代表的授权主体。Tooling 必须代理其中一个主体，不能自行获得权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Human,
    Ai,
}

/// Human 对受保护内容和强锁解除的显式授权。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct HumanAuthorization {
    pub modify_protected: bool,
    pub unlock_strong_lock: bool,
}

/// 由调用方描述的转移影响。0.3.2 的 patch validator 将负责从真实 patch 证明这些值。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TransitionEffects {
    pub content_changed: bool,
    pub structure_changed: bool,
    pub attachment_changed: bool,
}

impl TransitionEffects {
    pub fn is_unchanged(self) -> bool {
        !self.content_changed && !self.structure_changed && !self.attachment_changed
    }

    pub fn changes_protected_boundary(self) -> bool {
        self.content_changed || self.structure_changed || self.attachment_changed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionRequest<'a> {
    pub actor: Actor,
    pub from: Commitment,
    pub to: Commitment,
    pub effects: TransitionEffects,
    pub authorization: HumanAuthorization,
    pub challenge_reason: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionViolation {
    AiTransitionForbidden,
    ProtectedContentChanged,
    ProtectedStructureChanged,
    ProtectedAttachmentChanged,
    ChallengeReasonRequired,
    HumanAuthorizationRequired,
    StrongUnlockAuthorizationRequired,
}

pub type TransitionDecision = Result<(), TransitionViolation>;

/// 验证单个 commitment slot 的 Actor 转移。
///
/// 该 API 验证 0.3.0 的语义请求。它不验证源码 patch 是否诚实声明了 effects；
/// span、hash 和结构化 patch 的证明属于 0.3.1-0.3.2。
pub fn validate_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    match request.actor {
        Actor::Human => validate_human_transition(request),
        Actor::Ai => validate_ai_transition(request),
    }
}

fn validate_human_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    if request.from.lock_intent() == LockIntent::StrongLocked
        && request.to.lock_intent() != LockIntent::StrongLocked
        && !request.authorization.unlock_strong_lock
    {
        return Err(TransitionViolation::StrongUnlockAuthorizationRequired);
    }

    if request.from.protects_content()
        && request.effects.changes_protected_boundary()
        && !request.authorization.modify_protected
    {
        return Err(TransitionViolation::HumanAuthorizationRequired);
    }

    Ok(())
}

fn validate_ai_transition(request: &TransitionRequest<'_>) -> TransitionDecision {
    if !ai_transition_allowed(request.from, request.to) {
        return Err(TransitionViolation::AiTransitionForbidden);
    }

    if request.from.protects_content() {
        if request.effects.content_changed {
            return Err(TransitionViolation::ProtectedContentChanged);
        }
        if request.effects.structure_changed {
            return Err(TransitionViolation::ProtectedStructureChanged);
        }
        if request.effects.attachment_changed {
            return Err(TransitionViolation::ProtectedAttachmentChanged);
        }
    }

    if matches!(
        (request.from, request.to),
        (Commitment::Locked, Commitment::LockedQuestion)
            | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
    ) && request
        .challenge_reason
        .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err(TransitionViolation::ChallengeReasonRequired);
    }

    Ok(())
}

fn ai_transition_allowed(from: Commitment, to: Commitment) -> bool {
    matches!(
        (from, to),
        (Commitment::None, Commitment::Question)
            | (Commitment::Question, Commitment::Question)
            | (Commitment::QuestionQuestion, Commitment::Question)
            | (Commitment::QuestionQuestion, Commitment::None)
            | (Commitment::Locked, Commitment::LockedQuestion)
            | (Commitment::StrongLocked, Commitment::StrongLockedQuestion)
            | (
                Commitment::LockedQuestionQuestion,
                Commitment::LockedQuestion
            )
            | (
                Commitment::StrongLockedQuestionQuestion,
                Commitment::StrongLockedQuestion
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ReviewIntent, ReviewTarget};

    const STATES: [Commitment; 9] = [
        Commitment::None,
        Commitment::Question,
        Commitment::QuestionQuestion,
        Commitment::Locked,
        Commitment::LockedQuestion,
        Commitment::LockedQuestionQuestion,
        Commitment::StrongLocked,
        Commitment::StrongLockedQuestion,
        Commitment::StrongLockedQuestionQuestion,
    ];

    fn request(actor: Actor, from: Commitment, to: Commitment) -> TransitionRequest<'static> {
        TransitionRequest {
            actor,
            from,
            to,
            effects: TransitionEffects::default(),
            authorization: HumanAuthorization::default(),
            challenge_reason: Some("new evidence conflicts with the lock"),
        }
    }

    #[test]
    fn commitment_semantics_cover_all_nine_states() {
        let expected = [
            (
                LockIntent::Open,
                ReviewIntent::None,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Open,
                ReviewIntent::Review,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Open,
                ReviewIntent::Delegate,
                ReviewTarget::Content,
                false,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::None,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::Review,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::Locked,
                ReviewIntent::Delegate,
                ReviewTarget::Lock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::None,
                ReviewTarget::StrongLock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::Review,
                ReviewTarget::StrongLock,
                true,
            ),
            (
                LockIntent::StrongLocked,
                ReviewIntent::Delegate,
                ReviewTarget::StrongLock,
                true,
            ),
        ];

        for (state, (lock, review, target, protected)) in STATES.into_iter().zip(expected) {
            assert_eq!(state.lock_intent(), lock);
            assert_eq!(state.review_intent(), review);
            assert_eq!(state.review_target(), target);
            assert_eq!(state.protects_content(), protected);
            assert_eq!(state.is_delegated(), review == ReviewIntent::Delegate);
            assert_eq!(
                state.requires_human_decision(),
                review == ReviewIntent::Review
            );
        }

        assert!(Commitment::Locked.is_commit_ready());
        assert!(Commitment::StrongLocked.is_commit_ready());
        for state in STATES {
            if !matches!(state, Commitment::Locked | Commitment::StrongLocked) {
                assert!(!state.is_commit_ready(), "{state:?}");
            }
        }
    }

    #[test]
    fn ai_transition_matrix_is_exhaustive() {
        for from in STATES {
            for to in STATES {
                let decision = validate_transition(&request(Actor::Ai, from, to));
                assert_eq!(
                    decision.is_ok(),
                    ai_transition_allowed(from, to),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn human_transition_matrix_allows_state_changes_with_required_unlock() {
        for from in STATES {
            for to in STATES {
                let mut request = request(Actor::Human, from, to);
                request.authorization.unlock_strong_lock = true;
                assert!(validate_transition(&request).is_ok(), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn ai_lock_challenge_requires_reason_and_preserves_boundary() {
        let mut request = request(Actor::Ai, Commitment::Locked, Commitment::LockedQuestion);
        request.challenge_reason = None;
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ChallengeReasonRequired)
        );

        request.challenge_reason = Some("evidence");
        request.effects.content_changed = true;
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedContentChanged)
        );
        request.effects = TransitionEffects {
            structure_changed: true,
            ..TransitionEffects::default()
        };
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedStructureChanged)
        );
        request.effects = TransitionEffects {
            attachment_changed: true,
            ..TransitionEffects::default()
        };
        assert_eq!(
            validate_transition(&request),
            Err(TransitionViolation::ProtectedAttachmentChanged)
        );
    }

    #[test]
    fn human_protected_edits_and_strong_unlock_require_authorization() {
        let mut protected_edit = request(Actor::Human, Commitment::Locked, Commitment::Locked);
        protected_edit.effects.content_changed = true;
        assert_eq!(
            validate_transition(&protected_edit),
            Err(TransitionViolation::HumanAuthorizationRequired)
        );
        protected_edit.authorization.modify_protected = true;
        assert!(validate_transition(&protected_edit).is_ok());

        let mut unlock = request(Actor::Human, Commitment::StrongLocked, Commitment::Locked);
        assert_eq!(
            validate_transition(&unlock),
            Err(TransitionViolation::StrongUnlockAuthorizationRequired)
        );
        unlock.authorization.unlock_strong_lock = true;
        assert!(validate_transition(&unlock).is_ok());
    }
}
