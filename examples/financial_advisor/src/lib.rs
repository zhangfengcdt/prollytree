pub mod advisor;
pub mod benchmarks;
pub mod memory;
pub mod security;
pub mod validation;
pub mod visualization;

pub use advisor::{FinancialAdvisor, RecommendationType};
pub use memory::{MemoryConsistencyChecker, ValidatedMemory};
pub use security::SecurityMonitor;
pub use validation::{CrossReference, MemoryValidator, ValidationPolicy};

/// Re-export commonly used types
pub mod prelude {
    pub use super::advisor::*;
    pub use super::memory::ValidatedMemory;
    pub use super::security::SecurityMonitor;
    pub use super::validation::ValidationPolicy;
}
