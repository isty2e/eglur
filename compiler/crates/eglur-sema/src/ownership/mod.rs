mod analysis;
mod body;
mod diagnostics;
mod model;

pub use analysis::{OwnershipAnalysisError, analyze_ownership};
pub use model::{OwnershipBodyStatus, OwnershipBodySummary, OwnershipFacts, OwnershipRootFact};

use body::analyze_body;
use diagnostics::emit_ownership_diagnostics;
pub(crate) use model::{OwnershipAvailability, OwnershipFailure, OwnershipFailureKind};
