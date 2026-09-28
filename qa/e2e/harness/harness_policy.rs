//! Declarative policy validator harness for QA testing.
//!
//! Authoritative source: Spec Miner Survey § 4.3 and PROJECT.md.

use std::collections::{HashMap, HashSet};
use super::harness_models::{RemediationAction, Severity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyVerdict {
    Permitted,
    DisallowedAction(String),
    DisallowedUnit(String),
    RequiresConfirmation(Severity),
}

pub struct PolicyEngine {
    pub allowed_actions: HashSet<RemediationAction>,
    pub disallowed_units: HashSet<String>,
    pub require_confirmation_for: HashSet<Severity>,
    pub unit_allowed_actions: HashMap<String, HashSet<RemediationAction>>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        let mut allowed = HashSet::new();
        allowed.insert(RemediationAction::Restart);
        allowed.insert(RemediationAction::RestartWithBackoff);
        allowed.insert(RemediationAction::Reload);
        allowed.insert(RemediationAction::ResetFailed);

        let mut disallowed_units = HashSet::new();
        disallowed_units.insert("systemd-journald.service".to_string());
        disallowed_units.insert("dbus.service".to_string());
        disallowed_units.insert("systemd-logind.service".to_string());
        disallowed_units.insert("emergency.service".to_string());

        let mut require_conf = HashSet::new();
        require_conf.insert(Severity::Critical);

        Self {
            allowed_actions: allowed,
            disallowed_units,
            require_confirmation_for: require_conf,
            unit_allowed_actions: HashMap::new(),
        }
    }
}

impl PolicyEngine {
    pub fn evaluate(
        &self,
        unit: &str,
        action: RemediationAction,
        severity: Severity,
    ) -> PolicyVerdict {
        // 1. Critical systemd infrastructure blacklist
        if self.disallowed_units.contains(unit) {
            return PolicyVerdict::DisallowedUnit(unit.to_string());
        }

        // 2. Per-unit action whitelist override
        if let Some(unit_allowed) = self.unit_allowed_actions.get(unit) {
            if !unit_allowed.contains(&action) {
                return PolicyVerdict::DisallowedAction(format!("{:?}", action));
            }
        } else if !self.allowed_actions.contains(&action) {
            return PolicyVerdict::DisallowedAction(format!("{:?}", action));
        }

        // 3. Human confirmation gate
        if self.require_confirmation_for.contains(&severity) {
            return PolicyVerdict::RequiresConfirmation(severity);
        }

        PolicyVerdict::Permitted
    }
}
