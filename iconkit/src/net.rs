//! The only network access of the crate: one GET for text (the metadata
//! JSON), one for bytes (the SVG), one user-agent behind them. No async
//! runtime — a CLI that downloads a file wants `ureq` and nothing else.

use anyhow::{bail, Context, Result};

const UA: &str = concat!("iconkit/", env!("CARGO_PKG_VERSION"));

/// Body limit: the biggest metadata payload is under 4 MB, the biggest
/// SVG is a few kilobytes — 64 MB is a runaway ceiling, not a budget.
const MAX_BODY: u64 = 64 << 20;

/// GET a URL as text. Any non-2xx surfaces as an error naming the URL.
pub fn get_text(url: &str) -> Result<String> {
	let mut res =
		ureq::get(url).header("User-Agent", UA).call().with_context(|| format!("GET {url}"))?;
	res.body_mut()
		.with_config()
		.limit(MAX_BODY)
		.read_to_string()
		.with_context(|| format!("read body of {url}"))
}

/// GET a URL as bytes — same contract, for the SVG payload.
pub fn get_bytes(url: &str) -> Result<Vec<u8>> {
	let mut res =
		ureq::get(url).header("User-Agent", UA).call().with_context(|| format!("GET {url}"))?;
	let bytes = res
		.body_mut()
		.with_config()
		.limit(MAX_BODY)
		.read_to_vec()
		.with_context(|| format!("read body of {url}"))?;
	if bytes.is_empty() {
		bail!("GET {url} returned an empty body");
	}
	Ok(bytes)
}
