//! Persistent libp2p identity, so a node keeps the same peer id across restarts.

use std::path::Path;

use libp2p::identity::Keypair;

use crate::node::P2pError;

/// File name (inside the node data directory) holding the protobuf-encoded libp2p keypair.
pub const IDENTITY_FILE: &str = "p2p_identity.key";

/// Load the keypair at `path`, or generate an Ed25519 keypair and save it there (mode 0600 on
/// Unix). A file that exists but cannot be decoded is an error rather than silently replaced, so a
/// damaged key never changes the peer id without the operator noticing.
pub fn load_or_create(path: &Path) -> Result<Keypair, P2pError> {
    match std::fs::read(path) {
        Ok(bytes) => Keypair::from_protobuf_encoding(&bytes).map_err(|e| {
            P2pError::Network(format!(
                "P2P identity {} is unreadable ({e}); move it aside to generate a new peer id",
                path.display()
            ))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let key = Keypair::generate_ed25519();
            save(path, &key)?;
            tracing::info!(
                "P2P: generated new identity peer_id={} saved to {}",
                key.public().to_peer_id(),
                path.display()
            );
            Ok(key)
        }
        Err(e) => Err(P2pError::Network(format!(
            "P2P identity {}: {e}",
            path.display()
        ))),
    }
}

fn save(path: &Path, key: &Keypair) -> Result<(), P2pError> {
    let io = |e: std::io::Error| P2pError::Network(format!("P2P identity {}: {e}", path.display()));
    let bytes = key
        .to_protobuf_encoding()
        .map_err(|e| P2pError::Network(format!("P2P identity encode: {e}")))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("key.tmp");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(io)?;
        f.write_all(&bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable_across_loads() {
        let dir = std::env::temp_dir().join(format!(
            "boing-p2p-identity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join(IDENTITY_FILE);
        let a = load_or_create(&path).expect("create");
        let b = load_or_create(&path).expect("reload");
        assert_eq!(a.public().to_peer_id(), b.public().to_peer_id());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::write(&path, b"garbage").unwrap();
        assert!(
            load_or_create(&path).is_err(),
            "corrupt key must not be replaced silently"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
