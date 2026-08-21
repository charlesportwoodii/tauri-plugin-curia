use std::sync::Arc;

use curia::LogEvent;

// A sink renders a line; it does not decide what the line says. The consumer
// supplies the format, which is how the same sink serves JSON and human output.
pub type LineFormatter = Arc<dyn Fn(&LogEvent) -> String + Send + Sync>;
