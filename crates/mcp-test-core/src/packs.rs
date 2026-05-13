//! Versioned scenario packs shipped with the library (JSON under `resources/packs/`).

use crate::error::CoreError;
use crate::suite::{Scenario, ServerSpec, SessionMode, SuiteFile};

/// Load embedded scenarios for a named pack (`default` mirrors the starter conformance surface).
pub fn load_pack_scenarios(name: &str) -> Result<Vec<Scenario>, CoreError> {
    match name {
        "default" => serde_json::from_str(include_str!("../resources/packs/default.json"))
            .map_err(CoreError::Json),
        other => Err(CoreError::Handshake(format!(
            "unknown conformance pack: {other}"
        ))),
    }
}

/// Build a suite from an embedded pack plus a [`ServerSpec`].
pub fn suite_from_pack(server: ServerSpec, pack: &str) -> Result<SuiteFile, CoreError> {
    let scenarios = load_pack_scenarios(pack)?;
    Ok(SuiteFile {
        version: 3,
        session: SessionMode::default(),
        server,
        scenarios,
    })
}
