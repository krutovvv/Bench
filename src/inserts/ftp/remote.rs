//! A server's file kept as a local copy in the cache: where the copy came from.

use std::path::PathBuf;

/// Where a server's file is kept while open on this Mac.
pub fn cache_path(alias: &str, remote: &str) -> PathBuf {
    crate::ssh::home()
        .join("Library/Caches/bench/remote")
        .join(alias.replace('/', "_"))
        .join(remote.trim_start_matches('/'))
}

/// The server file a cache path is the local copy of: (alias, path on the server).
pub fn origin(local: &str) -> Option<(String, String)> {
    let base = cache_path("", "");
    let rest = local.strip_prefix(base.to_str()?)?.trim_start_matches('/');
    let (alias, path) = rest.split_once('/')?;
    Some((alias.to_string(), format!("/{path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_of_cache_paths() {
        let local = cache_path("dev", "/etc/nginx/conf.d/site.conf");
        assert_eq!(
            origin(local.to_str().unwrap()),
            Some(("dev".into(), "/etc/nginx/conf.d/site.conf".into()))
        );
        assert_eq!(origin("/Users/x/Desktop/a.txt"), None);
    }
}
