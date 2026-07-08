//! The standard.site publication-ownership proof: a static
//! `/.well-known/site.standard.publication` file whose body is the publication
//! record's AT-URI. The URI is fully derivable from the inputs at hand — the
//! account DID (`ITALIC_ATPROTO_DID`) + an rkey hashed from the site origin (see
//! [`crate::atproto::document::publication_uri`]) — so the proof is pure,
//! offline, and present in every build. Structurally
//! this mirrors the built-in sitemap/feed generators: a generated output with a
//! non-`.html` path that bypasses templating. Gated on `atproto.verification`
//! plus the DID/`site.url` inputs the derivation needs.

use crate::atproto::document;
use crate::build::Output;
use crate::config::Config;
use anyhow::Result;
use std::path::PathBuf;

/// Output path of the proof file, relative to `output_dir`.
const OUTPUT_PATH: &str = ".well-known/site.standard.publication";

/// Emit the `.well-known/site.standard.publication` proof, or nothing when
/// verification is off (`atproto.verification: false`) or the
/// `ITALIC_ATPROTO_DID` env var / `site.url` (the derivation inputs) are
/// missing. No `atproto:` block is needed — verification defaults on.
pub fn run(config: &Config, did: Option<&str>) -> Result<Vec<Output>> {
    if !config.atproto.verification {
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
    fn no_site_url_emits_nothing() {
        // A struct-built default config has verification on (the default) but no
        // `site.url`, so even with a DID the proof must not be emitted.
        let config = Config::default();
        assert!(run(&config, Some("did:plc:abc")).unwrap().is_empty());
    }

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
