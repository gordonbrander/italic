//! Site-level atproto identity, injected into the `site` context. The account
//! DID (`ITALIC_ATPROTO_DID`) and the publication record's AT-URI are fully
//! derivable from the inputs at hand — no network — so this pass computes them
//! directly and hands them to templates as `site.atproto_did` and
//! `site.atproto_publication_uri`.
//!
//! These are the site-wide counterparts to the per-doc `page.data.atproto_uri`
//! that [`crate::build::standard_link`] injects. They live on `site` rather than
//! on each doc because they describe the *site*, not the page: that way they are
//! present on every rendered page — the home page, archive pages, and docs
//! outside the publish collections all included — none of which carry a document
//! record of their own.
//!
//! The `at_meta` metadata filter renders them as `<meta name="at:me">` and
//! `<meta name="at:alternate">` (see `crate::tera_env::meta`); hand-rolled heads
//! can read them directly.
//!
//! Gated exactly like the other verification artifacts: `atproto.verification`
//! (default on) plus the DID and `site.url` the derivation needs. When the gate
//! closes this writes nothing at all, so a site that hand-sets `atproto_did:`
//! under `site:` in `config.yaml` keeps its value.

use crate::atproto::document;
use crate::config::Config;
use anyhow::Result;
use serde_yaml_ng::{Mapping, Value};

/// Site key the account DID is exposed under (read by templates as
/// `site.atproto_did`).
pub const DID_KEY: &str = "atproto_did";

/// Site key the publication record's AT-URI is exposed under (read by templates
/// as `site.atproto_publication_uri`).
pub const PUBLICATION_KEY: &str = "atproto_publication_uri";

pub fn run(config: &Config, did: Option<&str>, site: &mut Mapping) -> Result<()> {
    if !config.atproto.verification {
        return Ok(());
    }
    let (Some(did), Some(site_url)) = (did, &config.site_url) else {
        return Ok(());
    };
    inject(site, did, site_url);
    Ok(())
}

/// Set the DID and the derived publication AT-URI on the `site` mapping. Split
/// from [`run`] (which adds the config gating) so it is unit-testable.
fn inject(site: &mut Mapping, did: &str, site_url: &str) {
    site.insert(
        Value::String(DID_KEY.to_string()),
        Value::String(did.to_string()),
    );
    site.insert(
        Value::String(PUBLICATION_KEY.to_string()),
        Value::String(document::publication_uri(did, site_url)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const DID: &str = "did:plc:testabc";
    const SITE_URL: &str = "https://example.com";

    fn get<'a>(site: &'a Mapping, key: &str) -> Option<&'a str> {
        site.get(key).and_then(Value::as_str)
    }

    #[test]
    fn injects_did_and_derived_publication_uri() {
        let mut site = Mapping::new();
        inject(&mut site, DID, SITE_URL);
        assert_eq!(get(&site, DID_KEY), Some(DID));
        // Exactly the URI atproto would write: same publication_uri fn.
        let expected = document::publication_uri(DID, SITE_URL);
        assert_eq!(get(&site, PUBLICATION_KEY), Some(expected.as_str()));
    }

    #[test]
    fn no_did_leaves_a_hand_set_value_alone() {
        let mut site = Mapping::new();
        site.insert(
            Value::String(DID_KEY.to_string()),
            Value::String("did:plc:handset".to_string()),
        );
        run(&Config::default(), None, &mut site).unwrap();
        assert_eq!(get(&site, DID_KEY), Some("did:plc:handset"));
        assert!(get(&site, PUBLICATION_KEY).is_none());
    }

    #[test]
    fn no_site_url_injects_nothing() {
        // A struct-built default config has verification on (the default) but no
        // `site.url`, so even with a DID nothing is derivable.
        let mut site = Mapping::new();
        run(&Config::default(), Some(DID), &mut site).unwrap();
        assert!(site.is_empty());
    }

    #[test]
    fn verification_off_injects_nothing() {
        let mut config = Config {
            site_url: Some(SITE_URL.to_string()),
            ..Config::default()
        };
        config.atproto.verification = false;
        let mut site = Mapping::new();
        run(&config, Some(DID), &mut site).unwrap();
        assert!(site.is_empty());
    }
}
