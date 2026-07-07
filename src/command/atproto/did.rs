//! The `atproto did` verb: look up the DID behind a handle, so users can
//! set `ITALIC_ATPROTO_DID` without hunting through app settings. Resolution is
//! an unauthenticated `com.atproto.identity.resolveHandle` call against the
//! public AppView (which resolves any federated handle, not just accounts on one
//! PDS) — no config, no credentials. The bare DID goes to stdout (scriptable);
//! the export hint goes to stderr.

use anyhow::{Context, Result, anyhow};
use atrium_api::client::AtpServiceClient;
use atrium_xrpc_client::reqwest::ReqwestClient;

/// Public AppView host used for handle resolution.
const RESOLVER_HOST: &str = "https://public.api.bsky.app";

/// Resolve `handle` to its DID and print it. Tokio is confined to this function
/// (mirroring [`crate::atproto::status::run`]): it builds a current-thread
/// runtime and drives the async lookup to completion.
pub fn run(handle: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating tokio runtime")?;
    let did = runtime.block_on(resolve(handle))?;
    println!("{did}");
    eprintln!("hint: export ITALIC_ATPROTO_DID={did}");
    Ok(())
}

async fn resolve(handle: &str) -> Result<String> {
    let client = AtpServiceClient::new(ReqwestClient::new(RESOLVER_HOST));
    let params = atrium_api::com::atproto::identity::resolve_handle::ParametersData {
        handle: handle
            .parse()
            .map_err(|e| anyhow!("invalid handle `{handle}`: {e}"))?,
    }
    .into();
    let out = client
        .service
        .com
        .atproto
        .identity
        .resolve_handle(params)
        .await
        .map_err(|e| anyhow!("resolveHandle failed for {handle}: {e}"))?;
    Ok(out.data.did.as_str().to_string())
}
