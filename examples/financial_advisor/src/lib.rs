/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

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
