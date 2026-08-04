//! Compile-time trust policy for the official plugin allow-list.
//!
//! Public keys belong here; private signing keys must never enter the repository.
//! Every official repository entry must use a known key and pass Ed25519
//! verification after download.

use reading_core::plugin_repository::TrustedPluginKey;

pub const OFFICIAL_PLUGIN_KEYS: &[TrustedPluginKey<'static>] = &[TrustedPluginKey {
    key_id: "lnr-plugin-2026-01",
    public_key_base64: "IRMyKeK9u/gyoqB2sLm+GdQfTQGqHuKcD+gvkd05vcA=",
}];

pub const REQUIRE_OFFICIAL_PLUGIN_SIGNATURES: bool = true;
