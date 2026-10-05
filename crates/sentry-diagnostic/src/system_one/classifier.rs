//! Fast System One classification coordinator using candidate scoring.

use super::client::{DecisionCandidate, SystemOneClient};
use super::prompt_builder::build_decision_prompt;
use sentry_core::error::DiagnosticError;
use sentry_core::models::{DecisionTier, FaultClass, IncidentContext, SystemOneVerdict};

/// Fast classifier coordinating single-pass inference against runtimed.
#[derive(Debug, Clone)]
pub struct SystemOneClassifier {
    client: SystemOneClient,
    model: String,
}

impl Default for SystemOneClassifier {
    fn default() -> Self {
        Self::new(SystemOneClient::default(), "clef-flash")
    }
}

impl SystemOneClassifier {
    /// Creates a new classifier using the given Varlink client and model name.
    pub fn new(client: SystemOneClient, model: impl Into<String>) -> Self {
        Self {
            client,
            model: model.into(),
        }
    }

    /// Evaluates the incident against the 4-way taxonomy.
    pub async fn classify(
        &self,
        incident: &IncidentContext,
    ) -> Result<SystemOneVerdict, DiagnosticError> {
        let prompt = build_decision_prompt(incident);
        let candidates = vec![
            DecisionCandidate {
                name: FaultClass::TransientRestart.as_str().into(),
                token: FaultClass::TransientRestart.default_token().into(),
            },
            DecisionCandidate {
                name: FaultClass::ConfigDrift.as_str().into(),
                token: FaultClass::ConfigDrift.default_token().into(),
            },
            DecisionCandidate {
                name: FaultClass::DependencyFailure.as_str().into(),
                token: FaultClass::DependencyFailure.default_token().into(),
            },
            DecisionCandidate {
                name: FaultClass::ManualTriageRequired.as_str().into(),
                token: FaultClass::ManualTriageRequired.default_token().into(),
            },
        ];

        let raw = self
            .client
            .decide(&self.model, &prompt, &candidates, 1.0)
            .await?;

        let fault_class = FaultClass::parse_alias(&raw.winner)
            .unwrap_or(FaultClass::ManualTriageRequired);
        let tier = DecisionTier::from_confidence(raw.confidence);
        let explanation = format!(
            "System One classified '{}' as {} (confidence: {:.3}, margin: {:.3}, entropy: {:.3})",
            incident.unit,
            fault_class.as_str(),
            raw.confidence,
            raw.margin,
            raw.entropy
        );

        Ok(SystemOneVerdict {
            fault_class,
            confidence: raw.confidence,
            raw_probability: raw.raw_probability,
            margin: raw.margin,
            entropy: raw.entropy,
            tier,
            explanation,
        })
    }

    /// Resilient classification that falls back to Low-tier ManualTriage on client errors.
    pub async fn classify_resilient(&self, incident: &IncidentContext) -> SystemOneVerdict {
        match self.classify(incident).await {
            Ok(v) => v,
            Err(e) => SystemOneVerdict {
                fault_class: FaultClass::ManualTriageRequired,
                confidence: 0.0,
                raw_probability: 0.0,
                margin: 0.0,
                entropy: 1.0,
                tier: DecisionTier::Low,
                explanation: format!("Classifier fallback due to error: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry_core::models::DriverEvent;

    #[tokio::test]
    async fn test_resilient_fallback_on_unreachable_socket() {
        let fake_client = SystemOneClient::new("/nonexistent/socket");
        let classifier = SystemOneClassifier::new(fake_client, "test");
        let details = sentry_core::models::UnitFailedDetails {
            unit: "foo.service".into(),
            active_state: "failed".into(),
            sub_state: "failed".into(),
            result: Some("exit-code".into()),
            exec_status: Some(1),
            main_pid: None,
        };
        let incident = IncidentContext::new("foo.service", DriverEvent::UnitFailed(details));

        let verdict = classifier.classify_resilient(&incident).await;
        assert_eq!(verdict.tier, DecisionTier::Low);
        assert_eq!(verdict.fault_class, FaultClass::ManualTriageRequired);
        assert_eq!(verdict.confidence, 0.0);
    }
}
