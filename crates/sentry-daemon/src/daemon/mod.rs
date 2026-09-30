//! Supervisor daemon implementation, in-memory state, and incident processing.

pub mod audit_log;
pub mod event_multiplexer;
pub mod incident_manager;
pub mod pending_queue;
pub mod state;
pub mod supervisor;

pub use audit_log::AuditLogger;
pub use audit_log::AuditRecord;
pub use event_multiplexer::run_event_loop;
pub use incident_manager::IncidentManager;
pub use pending_queue::PendingIncident;
pub use pending_queue::PendingQueue;
pub use state::DaemonState;
pub use supervisor::run_supervisor;
