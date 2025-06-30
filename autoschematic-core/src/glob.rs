use std::path::{Component, Path};
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_addr_matches_filter_exact_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/iam/user/jon.ron");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_prefix_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_parent_prefix_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/*");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_parent() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("./aws/");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_region() {
        let addr = Path::new("./aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("aws/vpc/us-east-1");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_wrong_region() {
        let addr = Path::new("aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("./aws/vpc/us-east-2");
        assert_eq!(addr_matches_filter(addr, filter), false);
    }

    #[test]
    fn test_addr_matches_filter_region_wildcard() {
        let addr = Path::new("./aws/vpc/us-east-1/vpcs/main.ron");
        let filter = Path::new("aws/vpc/*/vpcs");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }


    #[test]
    fn test_addr_matches_filter_no_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("aws/vpc/./");
        assert_eq!(addr_matches_filter(addr, filter), false);
    }

    #[test]
    fn test_addr_matches_filter_wildcard_match() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/*/user");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }

    #[test]
    fn test_addr_matches_filter_wildcard_mismatch() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/iam/user/other");
        assert_eq!(addr_matches_filter(addr, filter), false);
    }

    #[test]
    fn test_addr_matches_filter_wildcard_at_start() {
        let addr = Path::new("aws/iam/user/jon.ron");
        let filter = Path::new("*/*/user/jon.ron");
        assert_eq!(addr_matches_filter(addr, filter), true);
    }
}

/// This function determines, for a given resource address, whether or not
/// the user-provided filter path excludes that resource address.
/// This includes * globs.
/// In zsh terms, the filter path behaves as if it always has "**/*" at the end.
/// For example:
///
/// addr_matches_filter("some/prefix". "aws/iam/user/jon.ron", "some/prefix") -> true
///
/// addr_matches_filter("some/prefix". "aws/iam/user/jon.ron", "some") -> true
///
/// addr_matches_filter("another/prefix". "aws/iam/user/jon.ron", "some") -> false
///
/// addr_matches_filter("another/prefix". "aws/iam/user/jon.ron", "*/prefix") -> true
///
/// addr_matches_filter("some/prefix". "aws/iam/user/jon.ron", "*/\*/aws") -> true
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
                if *full == filter {
                    continue;
                }

                if filter.to_str() == Some("*") {
                    continue;
                }

                return false;
            }
            _ => return false,
        }
    }

    true
}
