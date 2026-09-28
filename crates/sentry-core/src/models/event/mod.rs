//! Event-record models: driver events, incidents, coredump captures.

pub mod coredump;
pub mod driver_event;
pub mod incident_context;

pub use coredump::{CoredumpRecord, CoredumpXattrs};
pub use driver_event::{DriverEvent, JournalEntryDetails, UnitFailedDetails};
pub use incident_context::IncidentContext;
