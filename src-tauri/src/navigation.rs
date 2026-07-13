use url::Url;

pub(crate) fn is_workspace_navigation(workspace: &Url, target: &Url) -> bool {
    if target.scheme() == "about" && target.path() == "blank" {
        return true;
    }

    matches!(target.scheme(), "http" | "https")
        && workspace.scheme() == target.scheme()
        && workspace.host_str() == target.host_str()
        && workspace.port_or_known_default() == target.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use super::is_workspace_navigation;
    use url::Url;

    fn parsed(value: &str) -> Url {
        Url::parse(value).expect("test URL must be valid")
    }

    #[test]
    fn allows_navigation_within_the_configured_workspace_origin() {
        let workspace = parsed("https://ourea.example.com/app");
        let target = parsed("https://ourea.example.com/chat/42?tab=files");

        assert!(is_workspace_navigation(&workspace, &target));
    }

    #[test]
    fn treats_default_and_explicit_ports_as_the_same_origin() {
        let workspace = parsed("https://ourea.example.com");
        let target = parsed("https://ourea.example.com:443/settings");

        assert!(is_workspace_navigation(&workspace, &target));
    }

    #[test]
    fn rejects_subdomains_scheme_changes_and_non_web_urls() {
        let workspace = parsed("https://ourea.example.com");

        assert!(!is_workspace_navigation(
            &workspace,
            &parsed("https://docs.ourea.example.com")
        ));
        assert!(!is_workspace_navigation(
            &workspace,
            &parsed("http://ourea.example.com")
        ));
        assert!(!is_workspace_navigation(
            &workspace,
            &parsed("mailto:hello@example.com")
        ));
    }

    #[test]
    fn permits_about_blank_for_browser_managed_transitions() {
        let workspace = parsed("https://ourea.example.com");

        assert!(is_workspace_navigation(&workspace, &parsed("about:blank")));
    }
}
