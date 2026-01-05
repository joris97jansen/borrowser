pub(crate) fn resolve_relative_url(base_url: Option<&str>, href: &str) -> Option<String> {
    // If no base_url (e.g. initial about:blank), just pass through.
    let Some(base) = base_url else {
        return Some(href.to_string());
    };

    let base = url::Url::parse(base).ok()?;
    base.join(href).ok().map(|u| u.to_string())
}

