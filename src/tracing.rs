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

// TODO: fix this!!!
#![allow(dead_code)]
#![allow(unused_macros)]
#![allow(unexpected_cfgs)]

// Static variable for one-time initialization
pub(crate) static LOGGING_INIT: std::sync::Once = std::sync::Once::new();

// Debug macro for conditional compilation
macro_rules! debug {
    ($($t:tt)+) => {
        #[cfg(any(test, feature = "tracing", feature = "prod-logging"))]
        tracing::debug!($($t)+);
    };
}

// Trace macro for conditional compilation
macro_rules! trace {
    ($($t:tt)+) => {
        #[cfg(any(test, feature = "tracing", feature = "prod-logging"))]
        tracing::trace!($($t)+);
    };
}

// Macro to enable logging in test or production
macro_rules! enable_logging {
    () => {{
        LOGGING_INIT.call_once(|| {
            let subscriber = ::tracing_subscriber::FmtSubscriber::builder()
                .with_env_filter("trace")
                .with_writer(std::io::stdout)
                .finish();

            tracing::subscriber::set_global_default(subscriber).expect("failed to enable logging");
        });
    }};
}

// Test module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging() {
        // Enable logging for this test
        enable_logging!();

        debug!("This is a debug message in test.");
        trace!("This is a trace message in test.");

        // Your test code here
        assert_eq!(2 + 2, 4);
    }
}
