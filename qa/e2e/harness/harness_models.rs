#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RemediationAction {
    NoAction,
    Restart,
    RestartWithBackoff,
    Reload,
    ResetFailed,
    EscalateToAdmin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    Safe,
    Moderate,
    Hazardous,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    pub summary: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PsiStats {
    pub cpu_some_avg10: f64,
    pub memory_some_avg10: f64,
    pub memory_full_avg10: f64,
    pub io_some_avg10: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub journal_lines: Vec<String>,
    pub exit_codes: Vec<i32>,
    pub signals: Vec<String>,
    pub coredump_trace: Option<String>,
    pub psi_stats: Option<PsiStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedRemediation {
    pub action: RemediationAction,
    pub rationale: Option<String>,
    pub risk_level: RiskLevel,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticPayload {
    pub incident_id: String,
    pub timestamp: String,
    pub unit_name: String,
    pub root_cause: RootCause,
    pub evidence: Evidence,
    pub severity: Severity,
    pub proposed_remediation: ProposedRemediation,
}
