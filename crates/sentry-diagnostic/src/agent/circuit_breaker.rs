//! Autonomous agentic circuit breaker enforcing bounded iterations and execution timeout.

use std::time::{Duration, Instant};

/// Circuit breaker guarding against runaway autonomous agent loops.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Maximum permitted diagnostic iterations before tripping.
    pub max_iterations: usize,
    /// Absolute wall-clock timeout budget.
    pub timeout_duration: Duration,
    /// Loop commencement timestamp.
    pub start_time: Instant,
    /// Iterations executed so far.
    pub current_iteration: usize,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 30)
    }
}

impl CircuitBreaker {
    /// Create a circuit breaker with max iterations (default 5) and timeout (default 30s).
    pub fn new(max_iterations: usize, timeout_secs: u64) -> Self {
        Self {
            max_iterations,
            timeout_duration: Duration::from_secs(timeout_secs),
            start_time: Instant::now(),
            current_iteration: 0,
        }
    }

    /// Check if the circuit breaker has tripped without advancing iteration count.
    pub fn check(&self) -> Result<(), String> {
        if self.start_time.elapsed() >= self.timeout_duration {
            return Err(format!(
                "Agentic execution deadline exceeded: {:?} elapsed",
                self.start_time.elapsed()
            ));
        }
        if self.current_iteration >= self.max_iterations {
            return Err(format!(
                "Maximum agentic iterations reached: {}/{}",
                self.current_iteration, self.max_iterations
            ));
        }
        Ok(())
    }

    /// Advance iteration count and verify invariants.
    pub fn advance(&mut self) -> Result<usize, String> {
        self.check()?;
        let curr = self.current_iteration;
        self.current_iteration += 1;
        Ok(curr)
    }

    /// Elapsed time since circuit breaker instantiation.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_iteration_cap() {
        let mut breaker = CircuitBreaker::new(2, 60);
        assert!(breaker.advance().is_ok());
        assert!(breaker.advance().is_ok());
        assert!(breaker.advance().is_err());
    }

    #[test]
    fn test_circuit_breaker_timeout_zero() {
        let mut breaker = CircuitBreaker::new(5, 0);
        std::thread::sleep(Duration::from_millis(5));
        assert!(breaker.advance().is_err());
    }
}
