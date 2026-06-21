//! XRPC client + app-password auth for the PDS.
//!
//! v1 uses **app-password** auth (`com.atproto.server.createSession`), the
//! pragmatic near-term path; OAuth+DPoP is a fast-follow. The app password comes
//! only from the environment — **never** `config.yaml`. The non-secret host and
//! handle come from the environment too, falling back to the `publish:` config.
//! The [`Client`] wraps an `atrium-api` agent and exposes the three repo
//! operations both publish features need: [`Client::upload_blob`],
//! [`Client::put_record`] (create-or-update at a stable rkey, for
//! documents/publication), and [`Client::create_record`] (server-assigned rkey,
//! for create-once bsky posts).

use crate::publish::config::Publish;
use anyhow::{Result, anyhow};
use atrium_api::agent::Agent;
use atrium_api::agent::atp_agent::{CredentialSession, store::MemorySessionStore};
use atrium_api::types::{BlobRef, TryIntoUnknown};
use atrium_xrpc_client::reqwest::ReqwestClient;
use serde::Serialize;

/// Env var names. The app password is read only from here; the host/handle read
/// from here first, then fall back to the (non-secret) `publish:` config.
const ENV_PDS_HOST: &str = "ITALIC_ATPROTO_PDS_HOST";
const ENV_HANDLE: &str = "ITALIC_ATPROTO_HANDLE";
const ENV_APP_PASSWORD: &str = "ITALIC_ATPROTO_APP_PASSWORD";

type Session = CredentialSession<MemorySessionStore, ReqwestClient>;

/// Resolved connection secrets. The host/handle may come from config; the app
/// password must come from the environment.
pub struct Credentials {
    pub pds_host: String,
    pub handle: String,
    pub app_password: String,
}

impl Credentials {
    /// Resolve credentials from the environment, with a config fallback for the
    /// non-secret host/handle. The app password is secret: it is read only from
    /// `ITALIC_ATPROTO_APP_PASSWORD`, never config, and its absence is a hard
    /// error.
    pub fn load(publish: &Publish) -> Result<Credentials> {
        let pds_host = env(ENV_PDS_HOST).unwrap_or_else(|| publish.pds_host.clone());

        let handle = env(ENV_HANDLE)
            .or_else(|| publish.handle.clone())
            .ok_or_else(|| {
                anyhow!(
                    "no handle configured — set {ENV_HANDLE} or `publish.handle` in config.yaml"
                )
            })?;

        let app_password = env(ENV_APP_PASSWORD).ok_or_else(|| {
            anyhow!(
                "no app password — set {ENV_APP_PASSWORD} \
                 (create one at https://bsky.app/settings/app-passwords). \
                 Never put it in config.yaml."
            )
        })?;

        Ok(Credentials {
            pds_host,
            handle,
            app_password,
        })
    }
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// A reference to a written record: AT-URI + content hash.
pub struct WriteResult {
    pub uri: String,
    pub cid: String,
}

/// An authenticated PDS client.
pub struct Client {
    agent: Agent<Session>,
    /// The repo DID resolved from the session, used as the `repo` for writes.
    did: String,
}

impl Client {
    /// Authenticate with app-password auth and return a ready client. Resolves the
    /// account DID from the session.
    pub async fn login(creds: &Credentials) -> Result<Client> {
        let session = CredentialSession::new(
            ReqwestClient::new(&creds.pds_host),
            MemorySessionStore::default(),
        );
        let out = session
            .login(&creds.handle, &creds.app_password)
            .await
            .map_err(|e| anyhow!("createSession failed for {}: {e}", creds.handle))?;
        let did = out.data.did.as_str().to_string();
        let agent = Agent::new(session);
        Ok(Client { agent, did })
    }

    /// The authenticated account DID (`did:plc:…` / `did:web:…`).
    pub fn did(&self) -> &str {
        &self.did
    }

    /// Upload raw bytes as a blob and return the reference to embed in a record
    /// (e.g. `coverImage`, the bsky link-card `thumb`). Blobs are uploaded first,
    /// then referenced.
    pub async fn upload_blob(&self, bytes: Vec<u8>) -> Result<BlobRef> {
        let out = self
            .agent
            .api
            .com
            .atproto
            .repo
            .upload_blob(bytes)
            .await
            .map_err(|e| anyhow!("uploadBlob failed: {e}"))?;
        Ok(out.data.blob)
    }

    /// Create-or-update a record at a known `rkey` (stable, slug-derived).
    /// Re-publishing updates in place. Used for `site.standard.document` and the
    /// `site.standard.publication` record.
    pub async fn put_record(
        &self,
        collection: &str,
        rkey: &str,
        record: impl Serialize,
    ) -> Result<WriteResult> {
        let record = record
            .try_into_unknown()
            .map_err(|e| anyhow!("serializing record for {collection}: {e}"))?;
        let input = atrium_api::com::atproto::repo::put_record::InputData {
            collection: collection
                .parse()
                .map_err(|e| anyhow!("invalid collection NSID `{collection}`: {e}"))?,
            record,
            repo: self
                .did
                .parse()
                .map_err(|e| anyhow!("invalid repo DID `{}`: {e}", self.did))?,
            rkey: rkey
                .parse()
                .map_err(|e| anyhow!("invalid rkey `{rkey}`: {e}"))?,
            swap_commit: None,
            swap_record: None,
            validate: None,
        }
        .into();
        let out = self
            .agent
            .api
            .com
            .atproto
            .repo
            .put_record(input)
            .await
            .map_err(|e| anyhow!("putRecord {collection}/{rkey} failed: {e}"))?;
        Ok(WriteResult {
            uri: out.data.uri.clone(),
            cid: out.data.cid.as_ref().to_string(),
        })
    }

    /// Create a record with a server-assigned `rkey` (a TID). Used for
    /// `app.bsky.feed.post`, which is conventionally create-once.
    pub async fn create_record(
        &self,
        collection: &str,
        record: impl Serialize,
    ) -> Result<WriteResult> {
        let record = record
            .try_into_unknown()
            .map_err(|e| anyhow!("serializing record for {collection}: {e}"))?;
        let input = atrium_api::com::atproto::repo::create_record::InputData {
            collection: collection
                .parse()
                .map_err(|e| anyhow!("invalid collection NSID `{collection}`: {e}"))?,
            record,
            repo: self
                .did
                .parse()
                .map_err(|e| anyhow!("invalid repo DID `{}`: {e}", self.did))?,
            rkey: None,
            swap_commit: None,
            validate: None,
        }
        .into();
        let out = self
            .agent
            .api
            .com
            .atproto
            .repo
            .create_record(input)
            .await
            .map_err(|e| anyhow!("createRecord {collection} failed: {e}"))?;
        Ok(WriteResult {
            uri: out.data.uri.clone(),
            cid: out.data.cid.as_ref().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::config::{Bluesky, Publication, Publish};

    fn publish_config() -> Publish {
        Publish {
            pds_host: "https://bsky.social".into(),
            handle: Some("config.handle".into()),
            collection: "all".into(),
            verification: true,
            publication: Publication::default(),
            bluesky: Bluesky::default(),
        }
    }

    // Credential resolution mutates process-global env vars, so the cases share
    // one test to run sequentially rather than racing across parallel test
    // threads. Each step sets exactly the vars it needs and clears the rest.
    #[test]
    fn resolve_credentials() {
        let clear = || unsafe {
            std::env::remove_var(ENV_PDS_HOST);
            std::env::remove_var(ENV_HANDLE);
            std::env::remove_var(ENV_APP_PASSWORD);
        };

        // No app password anywhere → hard error pointing at the env var.
        clear();
        let err = format!(
            "{:#}",
            Credentials::load(&publish_config())
                .err()
                .expect("should error without a password")
        );
        assert!(err.contains("app password"), "{err}");

        // Password from env; host/handle fall back to config.
        clear();
        unsafe {
            std::env::set_var(ENV_APP_PASSWORD, "pw");
        }
        let creds = Credentials::load(&publish_config()).unwrap();
        assert_eq!(creds.handle, "config.handle");
        assert_eq!(creds.app_password, "pw");
        assert_eq!(creds.pds_host, "https://bsky.social");

        // Env overrides the config handle.
        unsafe {
            std::env::set_var(ENV_HANDLE, "env.handle");
        }
        let creds = Credentials::load(&publish_config()).unwrap();
        assert_eq!(creds.handle, "env.handle");

        clear();
    }
}
