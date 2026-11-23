use std::path::{Path, PathBuf};

use anyhow::bail;
use autoschematic_core::{config::AutoschematicConfig, glob::addr_matches_filter, util::split_prefix_addr};

pub fn select_paths(
    config: &AutoschematicConfig,
    paths: Vec<PathBuf>,
    prefix_filter: Option<&str>,
    connector_filter: Option<&str>,
    subpath_filter: Option<&str>,
) -> anyhow::Result<Vec<PathBuf>> {
    if let Some(prefix) = prefix_filter
        && !config.prefixes.contains_key(prefix)
    {
        bail!("Unknown prefix: {prefix}");
    }

    if let Some(name) = connector_filter
        && !config.prefixes.iter().any(|(prefix, def)| {
            prefix_filter.is_none_or(|filter| filter == prefix) && def.connectors.iter().any(|c| c.shortname == name)
        })
    {
        bail!("Unknown connector in selected prefixes: {name}");
    }

    Ok(paths
        .into_iter()
        .filter(|path| {
            let Some((prefix, addr)) = split_prefix_addr(config, path) else {
                return false;
            };
            prefix_filter.is_none_or(|filter| prefix == Path::new(filter))
                && subpath_filter.is_none_or(|filter| addr_matches_filter(&addr, Path::new(filter)))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use autoschematic_core::util::RON;

    #[test]
    fn filters_prefix_and_relative_subpath_before_loading_connectors() {
        let config = RON
            .from_str("AutoschematicConfig(prefixes: {\"prod\": Prefix(connectors: []), \"dev\": Prefix(connectors: [])})")
            .unwrap();
        let paths = vec![
            PathBuf::from("prod/k8s/deployment/site.yaml"),
            PathBuf::from("dev/k8s/deployment/site.yaml"),
            PathBuf::from("prod/k8s/service/site.yaml"),
            PathBuf::from("README.md"),
        ];
        let selected = select_paths(&config, paths, Some("prod"), None, Some("k8s/deployment/*.yaml")).unwrap();
        assert_eq!(selected, vec![PathBuf::from("prod/k8s/deployment/site.yaml")]);
        assert!(select_paths(&config, vec![], Some("typo"), None, None).is_err());
    }
}
