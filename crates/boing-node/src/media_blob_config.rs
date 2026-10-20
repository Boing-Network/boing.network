//! Media blob activation: `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` or CLI.
//!
//! Unset = feature off (PutBlob rejected). All validators must agree on the same height.

/// Resolve activation height from CLI override or env. `None` = media blobs disabled.
pub fn resolve_media_blobs_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    match std::env::var("BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT") {
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            let h: u64 = s
                .parse()
                .map_err(|_| format!("invalid BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT: {s}"))?;
            Ok(Some(h))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
