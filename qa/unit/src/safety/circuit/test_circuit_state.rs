//! Unit tests for CircuitState transitions and snapshot helpers.

use sentry_safety::circuit::CircuitState;
use std::time::{Duration, Instant};

#[test]
fn test_circuit_state_allows_remediation() {
    let now = Instant::now();
    assert!(CircuitState::Closed.allows_remediation());
    assert!(CircuitState::HalfOpen.allows_remediation());

    let open = CircuitState::Open {
        tripped_at: now,
        cooldown: Duration::from_secs(60),
        failure_count: 3,
    };
    assert!(!open.allows_remediation());

    let locked = CircuitState::PermanentlyLocked {
        locked_at: now,
        flap_trips: 3,
    };
    assert!(!locked.allows_remediation());
}

#[test]
fn test_circuit_state_labels() {
    let now = Instant::now();
    assert_eq!(CircuitState::Closed.label(), "CLOSED");
    assert_eq!(CircuitState::HalfOpen.label(), "HALF_OPEN");

    let open = CircuitState::Open {
        tripped_at: now,
        cooldown: Duration::from_secs(60),
        failure_count: 3,
    };
    assert_eq!(open.label(), "OPEN");

    let locked = CircuitState::PermanentlyLocked {
        locked_at: now,
        flap_trips: 3,
    };
    assert_eq!(locked.label(), "PERMANENTLY_LOCKED");
}
