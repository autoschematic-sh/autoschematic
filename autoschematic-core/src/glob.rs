use std::path::{Component, Path};

fn component_matches(value: &std::ffi::OsStr, pattern: &std::ffi::OsStr) -> bool {
    if value == pattern || pattern == "*" {
        return true;
    }

    let (Some(value), Some(pattern)) = (value.to_str(), pattern.to_str()) else {
        return false;
    };

    let value: Vec<char> = value.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let mut matches = vec![false; value.len() + 1];
    matches[0] = true;

    for pattern_char in pattern {
        if pattern_char == '*' {
            for i in 1..=value.len() {
                matches[i] = matches[i] || matches[i - 1];
            }
        } else {
            for i in (1..=value.len()).rev() {
                matches[i] = matches[i - 1] && value[i - 1] == pattern_char;
            }
            matches[0] = false;
        }
    }

    matches[value.len()]
}

/// This function determines, for a given resource address, whether or not
/// the user-provided filter path excludes that resource address.
/// This includes * globs.
/// In zsh terms, the filter path behaves as if it always has "**/*" at the end.
/// For example:
///
/// addr_matches_filter("some/prefix", "some/prefix") -> true
///
/// addr_matches_filter("some/prefix", "some") -> true
///
/// addr_matches_filter("another/prefix", "some") -> false
///
/// addr_matches_filter("another/prefix"", "*/prefix") -> true
pub fn addr_matches_filter(addr: &Path, filter: &Path) -> bool {
    let full_path_components: Vec<Component<'_>> = addr.components().filter(|c| *c != Component::CurDir).collect();
    let filter_components: Vec<Component<'_>> = filter.components().filter(|c| *c != Component::CurDir).collect();

    // Filter can't possibly match.
    if filter_components.len() > full_path_components.len() {
        return false;
    }

    for (full_path_component, filter_component) in full_path_components.iter().zip(filter_components) {
        match (full_path_component, filter_component) {
            (Component::Normal(full), Component::Normal(filter)) => {
                if component_matches(full, filter) {
                    continue;
                }

                return false;
            }
            _ => return false,
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_addr_matches_filter_exact_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/iam/user/jon.ron");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_prefix_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_parent_prefix_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/*");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_parent() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_region() {
        let addr = Path::new("./aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("aws/vpc/us-east-1");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_wrong_region() {
        let addr = Path::new("aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("./aws/vpc/us-east-2");
        assert!(!addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_region_wildcard() {
        let addr = Path::new("./aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("aws/vpc/*/vpcs");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_no_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("aws/vpc/./");
        assert!(!addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_wildcard_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/*/user");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_wildcard_mismatch() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/iam/user/other");
        assert!(!addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_wildcard_at_start() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/*/user/jon.ron");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_filename_extension_wildcard() {
        let addr = Path::new("k8s/default/ns/rhythm-nebula/deployment/backend.yaml");
        let filter = Path::new("k8s/default/ns/rhythm-nebula/deployment/*.yaml");
        assert!(addr_matches_filter(addr, filter));
    }

    #[test]
    fn test_addr_matches_filter_embedded_wildcards() {
        assert!(component_matches("backend.yaml".as_ref(), "back*.y*".as_ref()));
        assert!(!component_matches("backend.yml".as_ref(), "back*.yaml".as_ref()));
    }
}
