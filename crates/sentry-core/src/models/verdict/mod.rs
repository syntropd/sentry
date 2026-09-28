//! Diagnostic-verdict models: payloads, remediations, severity.

pub mod diagnostic;
pub mod remediation;
pub mod severity;

pub use diagnostic::{DiagnosticPayload, Evidence, ProposedRemediation, RiskLevel, RootCause};
pub use remediation::RemediationAction;
pub use severity::Severity;
