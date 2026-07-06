//! The `atproto` verb group: everything that touches an ATProto PDS —
//! `publish` (sync records), `status` (read them back), and `did`
//! (handle → DID lookup). Each submodule exposes a `run(...)` entrypoint,
//! mirroring the flat verbs in [`crate::command`].

pub mod did;
pub mod publish;
pub mod status;
