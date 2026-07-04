//! The standard.site publication-ownership proof: a static
//! `/.well-known/site.standard.publication` file whose body is the publication
//! record's AT-URI. Pure, offline, deterministic — so it lives in `build`, not
//! the networked `publish` step — but it is gated on `publish.verification` and
//! only emitted once `italic publish` has bootstrapped the publication (the
//! AT-URI is read from the publish state file). Structurally this mirrors the
//! built-in sitemap/feed generators: a generated output with a non-`.html` path
//! that bypasses templating.

use crate::build::Output;
use crate::config::Config;
use crate::publish::state::{STATE_PATH, State};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Output path of the proof file, relative to `output_dir`.
const OUTPUT_PATH: &str = ".well-known/site.standard.publication";

/// Emit the `.well-known/site.standard.publication` proof, or nothing when
/// verification is off, no `publish:` block exists, or the publication hasn't
/// been published yet.
pub fn run(config: &Config) -> Result<Vec<Output>> {
    let Some(publish) = &config.publish else {
        return Ok(Vec::new());
    };
    if !publish.verification {
        return Ok(Vec::new());
    }
    let state = State::load(Path::new(STATE_PATH))?;
    Ok(proof(&state))
}

/// The proof output(s) for a given state: one file when the publication has been
/// published, none otherwise. Split from [`run`] (which adds config gating + the
/// state-file read) so the mapping is unit-testable.
fn proof(state: &State) -> Vec<Output> {
    let Some(uri) = &state.publication_uri else {
        return Vec::new();
    };
    vec![Output {
        output_path: PathBuf::from(OUTPUT_PATH),
        content: format!("{uri}\n"),
        id_path: PathBuf::from(OUTPUT_PATH),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_publication_uri_emits_nothing() {
        assert!(proof(&State::default()).is_empty());
    }

    #[test]
    fn emits_uri_as_body() {
        let state = State {
            publication_uri: Some("at://did:plc:abc/site.standard.publication/self".into()),
            ..State::default()
        };
        let out = proof(&state);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].output_path, PathBuf::from(OUTPUT_PATH));
        assert_eq!(
            out[0].content,
            "at://did:plc:abc/site.standard.publication/self\n"
        );
    }
}
