//! Canonical parser-created attribute model.

use crate::names::{InternedLocalName, NameAtomId, NameInterner};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AttributeNamespace {
    None,
    Xml,
    Xmlns,
    XLink,
}

impl AttributeNamespace {
    pub const fn snapshot_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Xml => "xml",
            Self::Xmlns => "xmlns",
            Self::XLink => "xlink",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct QualifiedAttributeName {
    kind: QualifiedAttributeNameKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum QualifiedAttributeNameKind {
    Unqualified { local_name: InternedLocalName },
    Xml { local_name: InternedLocalName },
    XLink { local_name: InternedLocalName },
    XmlnsDefault,
    XmlnsPrefixed { local_name: InternedLocalName },
}

impl QualifiedAttributeName {
    pub fn unqualified(local_name: InternedLocalName) -> Self {
        Self {
            kind: QualifiedAttributeNameKind::Unqualified { local_name },
        }
    }

    pub(crate) fn xml(local_name: InternedLocalName) -> Self {
        Self {
            kind: QualifiedAttributeNameKind::Xml { local_name },
        }
    }

    pub(crate) fn xlink(local_name: InternedLocalName) -> Self {
        Self {
            kind: QualifiedAttributeNameKind::XLink { local_name },
        }
    }

    pub(crate) fn xmlns_default() -> Self {
        Self {
            kind: QualifiedAttributeNameKind::XmlnsDefault,
        }
    }

    pub(crate) fn xmlns_prefixed(local_name: InternedLocalName) -> Self {
        Self {
            kind: QualifiedAttributeNameKind::XmlnsPrefixed { local_name },
        }
    }

    pub fn namespace(&self) -> AttributeNamespace {
        match &self.kind {
            QualifiedAttributeNameKind::Unqualified { .. } => AttributeNamespace::None,
            QualifiedAttributeNameKind::Xml { .. } => AttributeNamespace::Xml,
            QualifiedAttributeNameKind::XLink { .. } => AttributeNamespace::XLink,
            QualifiedAttributeNameKind::XmlnsDefault
            | QualifiedAttributeNameKind::XmlnsPrefixed { .. } => AttributeNamespace::Xmlns,
        }
    }

    pub fn prefix(&self) -> Option<&'static str> {
        match &self.kind {
            QualifiedAttributeNameKind::Unqualified { .. }
            | QualifiedAttributeNameKind::XmlnsDefault => None,
            QualifiedAttributeNameKind::Xml { .. } => Some("xml"),
            QualifiedAttributeNameKind::XLink { .. } => Some("xlink"),
            QualifiedAttributeNameKind::XmlnsPrefixed { .. } => Some("xmlns"),
        }
    }

    pub fn local_name(&self) -> &str {
        match &self.kind {
            QualifiedAttributeNameKind::Unqualified { local_name }
            | QualifiedAttributeNameKind::Xml { local_name }
            | QualifiedAttributeNameKind::XLink { local_name }
            | QualifiedAttributeNameKind::XmlnsPrefixed { local_name } => local_name.as_str(),
            QualifiedAttributeNameKind::XmlnsDefault => "xmlns",
        }
    }

    pub fn same_expanded_name(&self, other: &Self) -> bool {
        self.namespace() == other.namespace() && self.local_name() == other.local_name()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserCreatedAttribute {
    name: QualifiedAttributeName,
    value: String,
}

impl ParserCreatedAttribute {
    pub fn new(name: QualifiedAttributeName, value: String) -> Self {
        Self { name, value }
    }

    pub fn unqualified(names: &NameInterner, name: NameAtomId, value: String) -> Option<Self> {
        names.resolve_local_name(name).map(|local_name| Self {
            name: QualifiedAttributeName::unqualified(local_name),
            value,
        })
    }

    pub fn name(&self) -> &QualifiedAttributeName {
        &self.name
    }

    pub fn namespace(&self) -> AttributeNamespace {
        self.name.namespace()
    }

    pub fn prefix(&self) -> Option<&'static str> {
        self.name.prefix()
    }

    pub fn local_name(&self) -> &str {
        self.name.local_name()
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

pub fn parser_created_attribute_lists_equal_ordered(
    left: &[ParserCreatedAttribute],
    right: &[ParserCreatedAttribute],
) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_shapes_expose_only_valid_namespace_prefix_pairs() {
        let mut names = NameInterner::new();
        let lang = names.intern_exact("lang").unwrap();
        let href = names.intern_exact("href").unwrap();
        let xlink = names.intern_exact("xlink").unwrap();

        let xml = QualifiedAttributeName::xml(names.resolve_local_name(lang).unwrap());
        let xlink_name = QualifiedAttributeName::xlink(names.resolve_local_name(href).unwrap());
        let default_xmlns = QualifiedAttributeName::xmlns_default();
        let prefixed_xmlns =
            QualifiedAttributeName::xmlns_prefixed(names.resolve_local_name(xlink).unwrap());

        assert_eq!(
            (xml.namespace(), xml.prefix()),
            (AttributeNamespace::Xml, Some("xml"))
        );
        assert_eq!(
            (xlink_name.namespace(), xlink_name.prefix()),
            (AttributeNamespace::XLink, Some("xlink"))
        );
        assert_eq!(
            (default_xmlns.namespace(), default_xmlns.prefix()),
            (AttributeNamespace::Xmlns, None)
        );
        assert_eq!(
            (prefixed_xmlns.namespace(), prefixed_xmlns.prefix()),
            (AttributeNamespace::Xmlns, Some("xmlns"))
        );
    }
}
