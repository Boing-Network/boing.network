//! Resolve secrets from CLI args, files, env, or stdin — avoid putting seeds in argv/`ps`.
//!
//! Supported sources (first match wins per call):
//! - explicit hex / token string (may be `-` for stdin, or `@path` for a file)
//! - `--*-file` / `BOING_*_FILE` path
//! - env var holding the raw secret

use std::io::Read;
use std::path::Path;

/// Read a secret string. Prefer file / stdin over argv so process listings don't leak seeds.
pub fn read_secret_string(
    inline: Option<String>,
    file: Option<String>,
    env_inline: &str,
    env_file: &str,
    what: &str,
) -> anyhow::Result<String> {
    if let Some(v) = inline.filter(|s| !s.trim().is_empty()) {
        return materialize_secret_token(&v, what);
    }
    if let Some(path) = file
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var(env_file).ok().filter(|s| !s.trim().is_empty()))
    {
        return read_secret_file(Path::new(&path), what);
    }
    if let Ok(v) = std::env::var(env_inline) {
        if !v.trim().is_empty() {
            return Ok(v.trim().to_string());
        }
    }
    anyhow::bail!(
        "no {what}: pass a value, --*-file / @path, `-` (stdin), or set {env_inline} / {env_file}"
    );
}

fn materialize_secret_token(v: &str, what: &str) -> anyhow::Result<String> {
    let t = v.trim();
    if t == "-" {
        return read_secret_stdin(what);
    }
    if let Some(path) = t.strip_prefix('@') {
        return read_secret_file(Path::new(path), what);
    }
    Ok(t.to_string())
}

fn read_secret_file(path: &Path, what: &str) -> anyhow::Result<String> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        anyhow::anyhow!("cannot read {what} file {}: {e}", path.display())
    })?;
    let line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .ok_or_else(|| anyhow::anyhow!("{what} file {} is empty", path.display()))?;
    Ok(line.to_string())
}

fn read_secret_stdin(what: &str) -> anyhow::Result<String> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| anyhow::anyhow!("cannot read {what} from stdin: {e}"))?;
    let line = buf
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{what} stdin was empty"))?;
    Ok(line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn at_prefix_reads_file() {
        let dir = tempfile_dir();
        let path = dir.join("seed.txt");
        std::fs::write(&path, "  aabb\n# comment\n").unwrap();
        let s = materialize_secret_token(&format!("@{}", path.display()), "secret").unwrap();
        assert_eq!(s, "aabb");
    }

    #[test]
    fn file_skips_comments() {
        let dir = tempfile_dir();
        let path = dir.join("tok");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "# comment").unwrap();
        writeln!(f, "op-token-1").unwrap();
        let s = read_secret_file(&path, "token").unwrap();
        assert_eq!(s, "op-token-1");
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "boing-cli-secrets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
