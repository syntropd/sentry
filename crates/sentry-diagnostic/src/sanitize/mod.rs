//! Multi-stage resilient sanitization pipeline.

pub mod pipeline;
pub mod repair;
pub mod strip_markdown;

pub use pipeline::{DiagnosticSanitizer, SanitizationPipeline};
pub use repair::repair_json;
pub use strip_markdown::{slice_outermost_json, strip_markdown_fences};

