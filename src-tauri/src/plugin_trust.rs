//! Compile-time trust policy for the official plugin allow-list.
//!
//! Public keys belong here; private signing keys must never enter the repository.
//! Until a release key is provisioned, unsigned entries remain available only in
//! explicit manual allow-list mode. Any entry that claims a signature must still
//! use a known key and pass Ed25519 verification after download.

use reading_core::plugin_repository::TrustedPluginKey;

pub const OFFICIAL_PLUGIN_KEYS: &[TrustedPluginKey<'static>] = &[];
pub const REQUIRE_OFFICIAL_PLUGIN_SIGNATURES: bool = false;
