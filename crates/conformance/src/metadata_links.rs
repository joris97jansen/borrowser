//! Shared build/capture facts. Attribute values are parsed values, never resolved URLs.
use serde::{Deserialize, Serialize};

pub const HTML: &str = "http://www.w3.org/1999/xhtml";
pub const MAX_LINKS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkFact {
    pub namespace: String,
    pub local_name: String,
    pub attributes: Vec<AttributeFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeFact {
    pub namespace: String,
    pub prefix: String,
    pub local_name: String,
    pub value: String,
}

impl LinkFact {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.namespace.is_empty() && a.prefix.is_empty() && a.local_name == name)
            .map(|a| a.value.as_str())
    }

    pub fn relation(&self) -> Result<String, String> {
        let tokens: Vec<_> = self
            .attribute("rel")
            .unwrap_or("")
            .split(['\t', '\n', '\x0c', '\r', ' '])
            .filter(|s| !s.is_empty())
            .collect();
        if tokens.len() != 1 {
            return Err("metadata link requires exactly one relation token".into());
        }
        Ok(tokens[0].to_ascii_lowercase())
    }
}

pub fn validate(links: &[LinkFact]) -> Result<(), String> {
    if links.len() > MAX_LINKS {
        return Err("metadata link count limit".into());
    }
    let mut bytes = 0;
    for link in links {
        if link.namespace != HTML || link.local_name != "link" {
            return Err("metadata link namespace/name".into());
        }
        if !(2..=3).contains(&link.attributes.len()) {
            return Err("metadata link attributes".into());
        }
        let mut previous = None;
        for a in &link.attributes {
            if !a.namespace.is_empty()
                || !a.prefix.is_empty()
                || !matches!(a.local_name.as_str(), "rel" | "href" | "title")
            {
                return Err("unsupported metadata link attribute".into());
            }
            if previous.is_some_and(|p| p >= a.local_name.as_str()) {
                return Err("metadata link attributes must be sorted and unique".into());
            }
            previous = Some(a.local_name.as_str());
            bytes += a.value.len() + a.local_name.len();
        }
        bytes += link.namespace.len() + link.local_name.len();
        if bytes > 8192 {
            return Err("metadata link string limit".into());
        }
        if !matches!(
            link.relation()?.as_str(),
            "author" | "help" | "match" | "mismatch"
        ) {
            return Err("unsupported metadata link relation".into());
        }
        if link.attribute("href").is_none_or(str::is_empty) {
            return Err("metadata link requires href".into());
        }
        if matches!(link.relation()?.as_str(), "match" | "mismatch") {
            reference_filename(link.attribute("href").unwrap())?;
        }
    }
    Ok(())
}

pub fn reference_filename(href: &str) -> Result<(), String> {
    let Some(stem) = href.strip_suffix(".html") else {
        return Err("reference must be a same-directory .html filename".into());
    };
    if stem.is_empty()
        || !stem.as_bytes()[0].is_ascii_alphanumeric()
        || !stem
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
    {
        return Err("reference must be a lowercase same-directory filename".into());
    }
    Ok(())
}
