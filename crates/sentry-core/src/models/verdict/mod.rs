//! Diagnostic-verdict models: payloads, remediations, severity, and gang triage.

pub mod decision;
pub mod diagnostic;
pub mod gang_triage;
pub mod remediation;
pub mod severity;

pub use decision::{DecisionTier, FaultClass, SystemOneVerdict};
pub use diagnostic::{DiagnosticPayload, Evidence, ProposedRemediation, RiskLevel, RootCause};
pub use gang_triage::{
    triage_gang_crash, GangCrashContext, GangErrorCategory, GangTriageVerdict,
};
pub use remediation::RemediationAction;
pub use severity::Severity;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verdict_module_reexports() {
        assert_eq!(Severity::Critical.as_str(), "CRITICAL");
        assert_eq!(RemediationAction::NoAction.as_str(), "NO_ACTION");
    }
}
