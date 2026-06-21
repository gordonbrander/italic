//! XRPC client + app-password auth for the PDS.
//!
//! v1 uses **app-password** auth (`com.atproto.server.createSession`), the
//! pragmatic near-term path; OAuth+DPoP is a fast-follow. Secrets come from the
//! environment or a gitignored credentials file — **never** `config.yaml`. The
//! [`Client`] wraps an `atrium-api` agent and exposes the three repo operations
//! both publish features need: [`Client::upload_blob`], [`Client::put_record`]
//! (create-or-update at a stable rkey, for documents/publication), and
//! [`Client::create_record`] (server-assigned rkey, for create-once bsky posts).

use crate::publish::config::Publish;
use anyhow::{Context, Result, anyhow};
use atrium_api::agent::Agent;
use atrium_api::agent::atp_agent::{CredentialSession, store::MemorySessionStore};
use atrium_api::types::{BlobRef, TryIntoUnknown};
use atrium_xrpc_client::reqwest::ReqwestClient;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Env var names for the (non-config) secrets.
const ENV_PDS_HOST: &str = "ITALIC_ATPROTO_PDS_HOST";
const ENV_HANDLE: &str = "ITALIC_ATPROTO_HANDLE";
const ENV_APP_PASSWORD: &str = "ITALIC_ATPROTO_APP_PASSWORD";

/// Default location of the gitignored credentials file (KEY=VALUE lines).
pub const CREDENTIALS_PATH: &str = ".italic/credentials";

type Session = CredentialSession<MemorySessionStore, ReqwestClient>;

/// Resolved connection secrets. The host/handle may come from config; the app
/// password must come from the environment or the credentials file.
pub struct Credentials {
    pub pds_host: String,
    pub handle: String,
    pub app_password: String,
}

impl Credentials {
    /// Resolve credentials. Precedence per field is env var → credentials file →
    /// config (host/handle only). The app password is secret: it is read only
    /// from the env or the file, never config, and its absence is a hard error
    /// with a pointer to both sources.
    pub fn load(publish: &Publish, credentials_path: &Path) -> Result<Credentials> {
        let file = CredentialsFile::load(credentials_path)?;

        let pds_host = env(ENV_PDS_HOST)
            .or_else(|| file.get("pds_host"))
            .unwrap_or_else(|| publish.pds_host.clone());

        let handle = env(ENV_HANDLE)
            .or_else(|| file.get("handle"))
            .or_else(|| publish.handle.clone())
            .ok_or_else(|| {
                anyhow!(
                    "no handle configured — set `publish.handle` in config.yaml, \
                     {ENV_HANDLE}, or `handle` in {}",
                    credentials_path.display()
                )
            })?;

        let app_password = env(ENV_APP_PASSWORD)
            .or_else(|| file.get("app_password"))
            .ok_or_else(|| {
                anyhow!(
                    "no app password — set {ENV_APP_PASSWORD} or `app_password` in {} \
                     (create one at https://bsky.app/settings/app-passwords). \
                     Never put it in config.yaml.",
                    credentials_path.display()
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

/// A minimal `KEY=VALUE` credentials file parser. Blank lines and `#` comments
/// are ignored. Absent file → empty (env/config may still supply everything).
struct CredentialsFile {
    entries: Vec<(String, String)>,
}

impl CredentialsFile {
    fn load(path: &Path) -> Result<CredentialsFile> {
        if !path.exists() {
            return Ok(CredentialsFile {
                entries: Vec::new(),
            });
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading credentials {}", path.display()))?;
        let mut entries = Vec::new();
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                entries.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        Ok(CredentialsFile { entries })
    }

    fn get(&self, key: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    }
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
    use crate::test_util::{cleanup, tempdir};

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

    #[test]
    fn credentials_file_supplies_password_and_overrides_handle() {
        let dir = tempdir("creds");
        let path = dir.join("credentials");
        fs::write(
            &path,
            "# my creds\nhandle = file.handle\napp_password = hunter2\n",
        )
        .unwrap();
        // Ensure env doesn't leak in from the test environment.
        unsafe {
            std::env::remove_var(ENV_HANDLE);
            std::env::remove_var(ENV_APP_PASSWORD);
            std::env::remove_var(ENV_PDS_HOST);
        }
        let creds = Credentials::load(&publish_config(), &path).unwrap();
        assert_eq!(creds.handle, "file.handle");
        assert_eq!(creds.app_password, "hunter2");
        assert_eq!(creds.pds_host, "https://bsky.social");
        cleanup(&dir);
    }

    #[test]
    fn missing_password_errors_with_pointer() {
        let dir = tempdir("creds");
        let path = dir.join("credentials"); // does not exist
        unsafe {
            std::env::remove_var(ENV_APP_PASSWORD);
        }
        let err = format!(
            "{:#}",
            Credentials::load(&publish_config(), &path)
                .err()
                .expect("should error without a password")
        );
        assert!(err.contains("app password"), "{err}");
        cleanup(&dir);
    }

    #[test]
    fn handle_falls_back_to_config() {
        let dir = tempdir("creds");
        let path = dir.join("credentials");
        fs::write(&path, "app_password = pw\n").unwrap();
        unsafe {
            std::env::remove_var(ENV_HANDLE);
        }
        let creds = Credentials::load(&publish_config(), &path).unwrap();
        assert_eq!(creds.handle, "config.handle");
        cleanup(&dir);
    }
}
