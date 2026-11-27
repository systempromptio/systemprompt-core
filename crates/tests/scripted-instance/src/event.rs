//! What a test can wait for.

use crate::record::{RequestRecord, Route};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Heartbeat,
    Telemetry,
    Verdict,
    ModelCall,
    ManifestFetch,
    Receipt,
    Route(Route),
}

impl Event {
    pub fn matches(self, record: &RequestRecord) -> bool {
        match self {
            Self::Heartbeat => record.route == Route::Heartbeat,
            Self::Telemetry => record.route == Route::Telemetry,
            Self::Verdict => record.route == Route::Verdict,
            Self::ModelCall => matches!(record.route, Route::Messages | Route::ChatCompletions),
            Self::ManifestFetch => record.route == Route::Manifest,
            Self::Receipt => record.route == Route::Receipt,
            Self::Route(route) => record.route == route,
        }
    }
}
