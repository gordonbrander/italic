//! The standard.site publication-ownership proof: a static
//! `/.well-known/site.standard.publication` file whose body is the publication
//! record's AT-URI. The URI is fully derivable from the inputs at hand — the
//! account DID (`ITALIC_ATPROTO_DID`) + an rkey hashed from the site origin (see
//! [`crate::publish::document::publication_uri`]) — so the proof is pure,
//! offline, and present in every build; no publish state is read. Structurally
//! this mirrors the built-in sitemap/feed generators: a generated output with a
//! non-`.html` path that bypasses templating. Gated on `publish.verification`
//! plus the DID/`site.url` inputs the derivation needs.

use crate::build::Output;
use crate::config::Config;
use crate::publish::document;
use anyhow::Result;
use std::path::PathBuf;

/// Output path of the proof file, relative to `output_dir`.
const OUTPUT_PATH: &str = ".well-known/site.standard.publication";

/// Emit the `.well-known/site.standard.publication` proof, or nothing when
/// verification is off, no `publish:` block exists, or the `ITALIC_ATPROTO_DID`
/// env var / `site.url` (the derivation inputs) are missing.
pub fn run(config: &Config, did: Option<&str>) -> Result<Vec<Output>> {
    let Some(publish) = &config.publish else {
        return Ok(Vec::new());
    };
    if !publish.verification {
        return Ok(Vec::new());
    }
    let (Some(did), Some(site_url)) = (did, &config.site_url) else {
        return Ok(Vec::new());
    };
    Ok(proof(did, site_url))
}

/// The proof output: one file carrying the derived publication AT-URI. Split
/// from [`run`] (which adds the config gating) so the mapping is unit-testable.
fn proof(did: &str, site_url: &str) -> Vec<Output> {
    vec![Output {
        output_path: PathBuf::from(OUTPUT_PATH),
        content: format!("{}\n", document::publication_uri(did, site_url)),
        id_path: PathBuf::from(OUTPUT_PATH),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_derives_publication_uri() {
        let outputs = proof("did:plc:abc", "https://example.com");
        assert_eq!(outputs.len(), 1);
        assert_eq!(
            outputs[0].output_path,
            PathBuf::from(".well-known/site.standard.publication")
        );
        assert_eq!(
            outputs[0].content,
            format!(
                "at://did:plc:abc/site.standard.publication/{}\n",
                document::publication_rkey("https://example.com")
            )
        );
    }
}
