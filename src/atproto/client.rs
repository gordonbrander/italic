//! XRPC client + app-password auth for the PDS.
//!
//! v1 uses **app-password** auth (`com.atproto.server.createSession`), the
//! pragmatic near-term path; OAuth+DPoP is a fast-follow. The account identity
//! is the **DID** (`ITALIC_ATPROTO_DID`) — createSession accepts a DID as the
//! identifier, and the same env var drives the build-time verification
//! artifacts, so there is a single source of truth. The app password comes only
//! from the environment — **never** `config.yaml`. The non-secret host comes
//! from the environment too, falling back to the `atproto:` config.
//! The [`Client`] wraps an `atrium-api` agent and exposes the repo operations
//! publishing needs: [`Client::upload_blob`], [`Client::put_record`]
//! (create-or-update at a stable rkey, for documents/publication), and
//! [`Client::list_records`] (the read side, for `status`).

use crate::atproto::config::Atproto;
use anyhow::{Result, anyhow};
use atrium_api::agent::Agent;
use atrium_api::agent::atp_agent::{CredentialSession, store::MemorySessionStore};
use atrium_api::types::{BlobRef, TryIntoUnknown};
use atrium_xrpc_client::reqwest::ReqwestClient;
use serde::Serialize;

/// Env var names. The app password and DID are read only from here; the host
/// reads from here first, then falls back to the (non-secret) `atproto:` config.
const ENV_PDS_HOST: &str = "ITALIC_ATPROTO_PDS_HOST";
const ENV_DID: &str = "ITALIC_ATPROTO_DID";
const ENV_APP_PASSWORD: &str = "ITALIC_ATPROTO_APP_PASSWORD";

type Session = CredentialSession<MemorySessionStore, ReqwestClient>;

/// Read the account DID from `ITALIC_ATPROTO_DID`. `Ok(None)` when unset (build
/// treats that as "skip verification artifacts"); a set-but-malformed value is a
/// hard error so a pasted handle fails loudly instead of minting bogus AT-URIs.
pub fn env_did() -> Result<Option<String>> {
    match env(ENV_DID) {
        None => Ok(None),
        Some(did) if did.starts_with("did:") => Ok(Some(did)),
        Some(other) => Err(anyhow!(
            "{ENV_DID} must be a DID like `did:plc:…` (got `{other}`) — \
             run `italic atproto did {other}` to look yours up"
        )),
    }
}

/// Resolved connection secrets. The host may come from config; the DID and app
/// password must come from the environment.
pub struct Credentials {
    pub pds_host: String,
    pub did: String,
    pub app_password: String,
}

impl Credentials {
    /// Resolve credentials from the environment, with a config fallback for the
    /// non-secret host. The DID is read only from `ITALIC_ATPROTO_DID` and the
    /// app password only from `ITALIC_ATPROTO_APP_PASSWORD` (secret — never
    /// config); either one missing is a hard error.
    pub fn load(atproto: &Atproto) -> Result<Credentials> {
        let pds_host = env(ENV_PDS_HOST).unwrap_or_else(|| atproto.pds_host.clone());

        let did = env_did()?.ok_or_else(|| {
            anyhow!(
                "no DID — set {ENV_DID} \
                 (run `italic atproto did <your-handle>` to look it up)"
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
            did,
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
    /// The account handle resolved from the session, for friendly display.
    handle: String,
}

impl Client {
    /// Authenticate with app-password auth and return a ready client. The DID is
    /// the createSession identifier; the session echoes back the DID and handle.
    pub async fn login(creds: &Credentials) -> Result<Client> {
        let session = CredentialSession::new(
            ReqwestClient::new(&creds.pds_host),
            MemorySessionStore::default(),
        );
        let out = session
            .login(&creds.did, &creds.app_password)
            .await
            .map_err(|e| anyhow!("createSession failed for {}: {e}", creds.did))?;
        let did = out.data.did.as_str().to_string();
        let handle = out.data.handle.as_str().to_string();
        let agent = Agent::new(session);
        Ok(Client { agent, did, handle })
    }

    /// The authenticated account DID (`did:plc:…` / `did:web:…`).
    pub fn did(&self) -> &str {
        &self.did
    }

    /// The authenticated account handle, as reported by the session.
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Upload raw bytes as a blob and return the reference to embed in a record
    /// (e.g. `coverImage`). Blobs are uploaded first, then referenced.
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

    /// Create-or-update a record at a known `rkey` (a stable hash of the
    /// document's canonical URL / the site origin). Re-publishing updates in
    /// place. Used for `site.standard.document` and the
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

    /// Create a record with a PDS-assigned TID rkey (`rkey: None`). Used for
    /// `app.bsky.feed.post`, whose lexicon requires TID keys — posts are
    /// create-once, and the returned uri/cid are persisted in
    /// `.italic/bsky.yaml` (see [`crate::atproto::state`]).
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

    /// List every record in `collection` from the authenticated repo, following
    /// the pagination cursor to the end.
    ///
    /// Read-only — used by `italic atproto status`, which treats the PDS as the
    /// source of truth for what is published.
    pub async fn list_records(
        &self,
        collection: &str,
    ) -> Result<Vec<atrium_api::com::atproto::repo::list_records::Record>> {
        let mut records = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = atrium_api::com::atproto::repo::list_records::ParametersData {
                collection: collection
                    .parse()
                    .map_err(|e| anyhow!("invalid collection NSID `{collection}`: {e}"))?,
                cursor: cursor.clone(),
                limit: None,
                repo: self
                    .did
                    .parse()
                    .map_err(|e| anyhow!("invalid repo DID `{}`: {e}", self.did))?,
                reverse: None,
            }
            .into();
            let out = self
                .agent
                .api
                .com
                .atproto
                .repo
                .list_records(params)
                .await
                .map_err(|e| anyhow!("listRecords {collection} failed: {e}"))?;
            // Guard against a server that returns a cursor with an empty page,
            // which would otherwise loop forever.
            let page_empty = out.data.records.is_empty();
            records.extend(out.data.records);
            cursor = out.data.cursor.clone();
            if cursor.is_none() || page_empty {
                break;
            }
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atproto::config::Atproto;

    fn publish_config() -> Atproto {
        Atproto::default()
    }

    // Credential resolution mutates process-global env vars, so the cases share
    // one test to run sequentially rather than racing across parallel test
    // threads. Each step sets exactly the vars it needs and clears the rest.
    #[test]
    fn resolve_credentials() {
        let clear = || unsafe {
            std::env::remove_var(ENV_PDS_HOST);
            std::env::remove_var(ENV_DID);
            std::env::remove_var(ENV_APP_PASSWORD);
        };

        // No DID anywhere → hard error pointing at the env var.
        clear();
        unsafe {
            std::env::set_var(ENV_APP_PASSWORD, "pw");
        }
        let err = format!(
            "{:#}",
            Credentials::load(&publish_config())
                .err()
                .expect("should error without a DID")
        );
        assert!(err.contains(ENV_DID), "{err}");

        // A handle pasted into the DID slot fails loudly.
        unsafe {
            std::env::set_var(ENV_DID, "alice.example.com");
        }
        let err = format!(
            "{:#}",
            Credentials::load(&publish_config())
                .err()
                .expect("should reject a non-DID value")
        );
        assert!(err.contains("must be a DID"), "{err}");

        // No app password → hard error pointing at the env var.
        clear();
        unsafe {
            std::env::set_var(ENV_DID, "did:plc:abc");
        }
        let err = format!(
            "{:#}",
            Credentials::load(&publish_config())
                .err()
                .expect("should error without a password")
        );
        assert!(err.contains("app password"), "{err}");

        // DID + password from env; host falls back to config.
        unsafe {
            std::env::set_var(ENV_APP_PASSWORD, "pw");
        }
        let creds = Credentials::load(&publish_config()).unwrap();
        assert_eq!(creds.did, "did:plc:abc");
        assert_eq!(creds.app_password, "pw");
        assert_eq!(creds.pds_host, "https://bsky.social");

        // Env overrides the config host.
        unsafe {
            std::env::set_var(ENV_PDS_HOST, "https://pds.example");
        }
        let creds = Credentials::load(&publish_config()).unwrap();
        assert_eq!(creds.pds_host, "https://pds.example");

        clear();
    }
}
