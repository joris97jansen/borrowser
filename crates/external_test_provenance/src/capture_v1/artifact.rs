use std::cmp::Ordering;
use std::fmt;

use crate::allocation::{
    ProductionReservation, ReservationPolicy, ReservationSite, try_reserve_string, try_reserve_vec,
};

use super::MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1;

const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const MATHML_NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalArtifactValidationError {
    TooLarge,
    InvalidUtf8,
    InvalidNewline,
    InvalidHeader,
    InvalidField,
    InvalidString,
    InvalidCount,
    InvalidStructure,
    InvalidNamespace,
    InvalidName,
    NonCanonicalAttributeOrder,
    DuplicateAttribute,
    Allocation,
    TrailingInput,
}

impl fmt::Display for ExternalArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "web-observable-dom-tree-v1 validation failed: {self:?}"
        )
    }
}

impl std::error::Error for ExternalArtifactValidationError {}

pub(super) fn validate_web_observable_dom_tree_v1(
    bytes: &[u8],
) -> Result<(), ExternalArtifactValidationError> {
    validate_web_observable_dom_tree_v1_with_policy(bytes, &mut ProductionReservation)
}

fn validate_web_observable_dom_tree_v1_with_policy(
    bytes: &[u8],
    reservation: &mut impl ReservationPolicy,
) -> Result<(), ExternalArtifactValidationError> {
    let byte_length =
        u64::try_from(bytes.len()).map_err(|_| ExternalArtifactValidationError::TooLarge)?;
    if byte_length > MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 {
        return Err(ExternalArtifactValidationError::TooLarge);
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") || bytes.contains(&b'\r') {
        return Err(ExternalArtifactValidationError::InvalidNewline);
    }
    let input =
        std::str::from_utf8(bytes).map_err(|_| ExternalArtifactValidationError::InvalidUtf8)?;
    let mut cursor = Cursor { input, offset: 0 };
    cursor
        .exact("format = \"web-observable-dom-tree-v1\"")
        .map_err(|_| ExternalArtifactValidationError::InvalidHeader)?;
    cursor
        .exact("root-count = 1")
        .map_err(|_| ExternalArtifactValidationError::InvalidHeader)?;

    let root = start_node(&mut cursor, true, reservation)?
        .ok_or(ExternalArtifactValidationError::InvalidStructure)?;
    let mut stack = Vec::new();
    try_reserve_vec(
        &mut stack,
        1,
        ReservationSite::ComparableDomStack,
        reservation,
    )
    .map_err(|_| ExternalArtifactValidationError::Allocation)?;
    stack.push(root);

    while let Some(frame) = stack.last_mut() {
        match frame {
            Frame::Document { remaining } => {
                if *remaining == 0 {
                    cursor.exact("node-end = \"document\"")?;
                    stack.pop();
                } else {
                    *remaining -= 1;
                    if let Some(child) = start_node(&mut cursor, false, reservation)? {
                        try_reserve_vec(
                            &mut stack,
                            1,
                            ReservationSite::ComparableDomStack,
                            reservation,
                        )
                        .map_err(|_| ExternalArtifactValidationError::Allocation)?;
                        stack.push(child);
                    }
                }
            }
            Frame::Element {
                namespace,
                local_name,
                ordinary_remaining,
                template_remaining,
                template_state,
            } => {
                if *ordinary_remaining > 0 {
                    *ordinary_remaining -= 1;
                    if let Some(child) = start_node(&mut cursor, false, reservation)? {
                        try_reserve_vec(
                            &mut stack,
                            1,
                            ReservationSite::ComparableDomStack,
                            reservation,
                        )
                        .map_err(|_| ExternalArtifactValidationError::Allocation)?;
                        stack.push(child);
                    }
                    continue;
                }
                if !*template_state {
                    let value = cursor.value("template-contents")?;
                    let is_template = namespace == HTML_NAMESPACE && local_name == "template";
                    match (is_template, value) {
                        (true, "\"present\"") => {
                            *template_remaining =
                                parse_count(cursor.value("template-child-count")?)?;
                        }
                        (false, "\"absent\"") => {}
                        _ => return Err(ExternalArtifactValidationError::InvalidStructure),
                    }
                    *template_state = true;
                    continue;
                }
                if *template_remaining > 0 {
                    *template_remaining -= 1;
                    if let Some(child) = start_node(&mut cursor, false, reservation)? {
                        try_reserve_vec(
                            &mut stack,
                            1,
                            ReservationSite::ComparableDomStack,
                            reservation,
                        )
                        .map_err(|_| ExternalArtifactValidationError::Allocation)?;
                        stack.push(child);
                    }
                    continue;
                }
                cursor.exact("node-end = \"element\"")?;
                stack.pop();
            }
        }
    }
    if cursor.offset != input.len() {
        return Err(ExternalArtifactValidationError::TrailingInput);
    }
    Ok(())
}

enum Frame {
    Document {
        remaining: u64,
    },
    Element {
        namespace: String,
        local_name: String,
        ordinary_remaining: u64,
        template_remaining: u64,
        template_state: bool,
    },
}

fn start_node(
    cursor: &mut Cursor<'_>,
    root: bool,
    reservation: &mut impl ReservationPolicy,
) -> Result<Option<Frame>, ExternalArtifactValidationError> {
    let kind = parse_string(cursor.value("node-begin")?, reservation)?;
    if root && kind != "document" || !root && kind == "document" {
        return Err(ExternalArtifactValidationError::InvalidStructure);
    }
    match kind.as_str() {
        "document" => Ok(Some(Frame::Document {
            remaining: parse_count(cursor.value("child-count")?)?,
        })),
        "element" => {
            let namespace = parse_string(cursor.value("namespace-uri")?, reservation)?;
            if !matches!(
                namespace.as_str(),
                HTML_NAMESPACE | SVG_NAMESPACE | MATHML_NAMESPACE
            ) {
                return Err(ExternalArtifactValidationError::InvalidNamespace);
            }
            let local_name = parse_string(cursor.value("local-name")?, reservation)?;
            if local_name.is_empty() {
                return Err(ExternalArtifactValidationError::InvalidName);
            }
            let attribute_count = parse_count(cursor.value("attribute-count")?)?;
            let mut previous: Option<AttributeKey> = None;
            for _ in 0..attribute_count {
                let key = parse_attribute(cursor, reservation)?;
                if let Some(previous) = &previous {
                    validate_attribute_progression(previous, &key)?;
                }
                previous = Some(key);
            }
            let ordinary_remaining = parse_count(cursor.value("child-count")?)?;
            Ok(Some(Frame::Element {
                namespace,
                local_name,
                ordinary_remaining,
                template_remaining: 0,
                template_state: false,
            }))
        }
        "document-type" => {
            if parse_string(cursor.value("name")?, reservation)?.is_empty() {
                return Err(ExternalArtifactValidationError::InvalidName);
            }
            parse_string(cursor.value("public-id")?, reservation)?;
            parse_string(cursor.value("system-id")?, reservation)?;
            cursor.exact("node-end = \"document-type\"")?;
            Ok(None)
        }
        "text" => {
            parse_string(cursor.value("data")?, reservation)?;
            cursor.exact("node-end = \"text\"")?;
            Ok(None)
        }
        "comment" => {
            parse_string(cursor.value("data")?, reservation)?;
            cursor.exact("node-end = \"comment\"")?;
            Ok(None)
        }
        "processing-instruction" => {
            let target = parse_string(cursor.value("target")?, reservation)?;
            if target.is_empty() {
                return Err(ExternalArtifactValidationError::InvalidName);
            }
            parse_string(cursor.value("data")?, reservation)?;
            cursor.exact("node-end = \"processing-instruction\"")?;
            Ok(None)
        }
        _ => Err(ExternalArtifactValidationError::InvalidStructure),
    }
}

#[derive(Clone)]
struct AttributeKey {
    namespace: Option<String>,
    local_name: String,
    prefix: Option<String>,
    qualified_name: String,
}

fn parse_attribute(
    cursor: &mut Cursor<'_>,
    reservation: &mut impl ReservationPolicy,
) -> Result<AttributeKey, ExternalArtifactValidationError> {
    cursor.exact("attribute-begin = true")?;
    let namespace = parse_optional_string(cursor.value("namespace-uri")?, reservation)?;
    let prefix = parse_optional_string(cursor.value("prefix")?, reservation)?;
    let local_name = parse_string(cursor.value("local-name")?, reservation)?;
    let qualified_name = parse_string(cursor.value("qualified-name")?, reservation)?;
    parse_string(cursor.value("value")?, reservation)?;
    cursor.exact("attribute-end = true")?;
    if local_name.is_empty() {
        return Err(ExternalArtifactValidationError::InvalidName);
    }
    match (namespace.as_deref(), prefix.as_deref()) {
        (None, None) => {}
        (Some(XML_NAMESPACE), Some("xml")) | (Some(XLINK_NAMESPACE), Some("xlink")) => {}
        (Some(XMLNS_NAMESPACE), None) if local_name == "xmlns" && qualified_name == "xmlns" => {}
        (Some(XMLNS_NAMESPACE), Some("xmlns")) => {}
        _ => return Err(ExternalArtifactValidationError::InvalidNamespace),
    }
    let qualified_name_is_valid = match &prefix {
        None => qualified_name == local_name,
        Some(prefix) => qualified_name
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_prefix(':'))
            .is_some_and(|suffix| suffix == local_name),
    };
    if !qualified_name_is_valid {
        return Err(ExternalArtifactValidationError::InvalidName);
    }
    Ok(AttributeKey {
        namespace,
        local_name,
        prefix,
        qualified_name,
    })
}

fn compare_attribute_key(left: &AttributeKey, right: &AttributeKey) -> Ordering {
    compare_optional(&left.namespace, &right.namespace)
        .then_with(|| left.local_name.as_bytes().cmp(right.local_name.as_bytes()))
        .then_with(|| compare_optional(&left.prefix, &right.prefix))
        .then_with(|| {
            left.qualified_name
                .as_bytes()
                .cmp(right.qualified_name.as_bytes())
        })
}

fn validate_attribute_progression(
    previous: &AttributeKey,
    current: &AttributeKey,
) -> Result<(), ExternalArtifactValidationError> {
    match compare_attribute_key(previous, current) {
        Ordering::Greater => Err(ExternalArtifactValidationError::NonCanonicalAttributeOrder),
        Ordering::Equal => Err(ExternalArtifactValidationError::DuplicateAttribute),
        Ordering::Less => Ok(()),
    }
}

fn compare_optional(left: &Option<String>, right: &Option<String>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(left), Some(right)) => left.as_bytes().cmp(right.as_bytes()),
    }
}

fn parse_optional_string(
    value: &str,
    reservation: &mut impl ReservationPolicy,
) -> Result<Option<String>, ExternalArtifactValidationError> {
    if value == "null" {
        Ok(None)
    } else {
        parse_string(value, reservation).map(Some)
    }
}

fn parse_count(value: &str) -> Result<u64, ExternalArtifactValidationError> {
    if value.is_empty()
        || value.starts_with('+')
        || value.starts_with('-')
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ExternalArtifactValidationError::InvalidCount);
    }
    value
        .parse()
        .map_err(|_| ExternalArtifactValidationError::InvalidCount)
}

fn parse_string(
    value: &str,
    reservation: &mut impl ReservationPolicy,
) -> Result<String, ExternalArtifactValidationError> {
    let body = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(ExternalArtifactValidationError::InvalidString)?;
    let mut output = String::new();
    try_reserve_string(
        &mut output,
        body.len(),
        ReservationSite::ComparableDomString,
        reservation,
    )
    .map_err(|_| ExternalArtifactValidationError::Allocation)?;
    let mut chars = body.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars
                .next()
                .ok_or(ExternalArtifactValidationError::InvalidString)?
            {
                '\\' => output.push('\\'),
                '"' => output.push('"'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                'u' => {
                    let digits = [chars.next(), chars.next(), chars.next(), chars.next()];
                    let [Some('0'), Some('0'), Some(high), Some(low)] = digits else {
                        return Err(ExternalArtifactValidationError::InvalidString);
                    };
                    let high = lower_hex(high)?;
                    let low = lower_hex(low)?;
                    let byte = high * 16 + low;
                    if !matches!(byte, 0..=8 | 11..=12 | 14..=31 | 127) {
                        return Err(ExternalArtifactValidationError::InvalidString);
                    }
                    output.push(char::from(byte));
                }
                _ => return Err(ExternalArtifactValidationError::InvalidString),
            }
        } else if character == '"' || matches!(character, '\u{0000}'..='\u{001f}' | '\u{007f}') {
            return Err(ExternalArtifactValidationError::InvalidString);
        } else {
            output.push(character);
        }
    }
    Ok(output)
}

fn lower_hex(character: char) -> Result<u8, ExternalArtifactValidationError> {
    match character {
        '0'..='9' => Ok(character as u8 - b'0'),
        'a'..='f' => Ok(character as u8 - b'a' + 10),
        _ => Err(ExternalArtifactValidationError::InvalidString),
    }
}

struct Cursor<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn line(&mut self) -> Result<&'a str, ExternalArtifactValidationError> {
        if self.offset >= self.input.len() {
            return Err(ExternalArtifactValidationError::InvalidStructure);
        }
        let rest = &self.input[self.offset..];
        let newline = rest
            .find('\n')
            .ok_or(ExternalArtifactValidationError::InvalidNewline)?;
        if newline == 0 {
            return Err(ExternalArtifactValidationError::InvalidNewline);
        }
        let line = &rest[..newline];
        let consumed = newline
            .checked_add(1)
            .ok_or(ExternalArtifactValidationError::InvalidCount)?;
        self.offset = self
            .offset
            .checked_add(consumed)
            .ok_or(ExternalArtifactValidationError::InvalidCount)?;
        Ok(line)
    }

    fn exact(&mut self, expected: &str) -> Result<(), ExternalArtifactValidationError> {
        if self.line()? == expected {
            Ok(())
        } else {
            Err(ExternalArtifactValidationError::InvalidField)
        }
    }

    fn value(&mut self, field: &str) -> Result<&'a str, ExternalArtifactValidationError> {
        let line = self.line()?;
        let value = line
            .strip_prefix(field)
            .and_then(|value| value.strip_prefix(" = "))
            .ok_or(ExternalArtifactValidationError::InvalidField)?;
        if value.is_empty() {
            return Err(ExternalArtifactValidationError::InvalidField);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocation::{RejectReservationAt, ReservationSite};

    const EMPTY: &[u8] = b"format = \"web-observable-dom-tree-v1\"\nroot-count = 1\nnode-begin = \"document\"\nchild-count = 0\nnode-end = \"document\"\n";

    #[test]
    fn empty_document_is_valid_and_trailing_or_crlf_input_is_not() {
        assert_eq!(validate_web_observable_dom_tree_v1(EMPTY), Ok(()));
        let mut trailing = EMPTY.to_vec();
        trailing.extend_from_slice(b"extra = true\n");
        assert_eq!(
            validate_web_observable_dom_tree_v1(&trailing),
            Err(ExternalArtifactValidationError::TrailingInput)
        );
        let mut crlf = EMPTY.to_vec();
        crlf[38] = b'\r';
        assert_eq!(
            validate_web_observable_dom_tree_v1(&crlf),
            Err(ExternalArtifactValidationError::InvalidNewline)
        );
    }

    fn valid_mixed_document() -> String {
        [
            "format = \"web-observable-dom-tree-v1\"",
            "root-count = 1",
            "node-begin = \"document\"",
            "child-count = 2",
            "node-begin = \"document-type\"",
            "name = \"html\"",
            "public-id = \"\"",
            "system-id = \"\"",
            "node-end = \"document-type\"",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/1999/xhtml\"",
            "local-name = \"html\"",
            "attribute-count = 2",
            "attribute-begin = true",
            "namespace-uri = null",
            "prefix = null",
            "local-name = \"id\"",
            "qualified-name = \"id\"",
            "value = \"root\"",
            "attribute-end = true",
            "attribute-begin = true",
            "namespace-uri = \"http://www.w3.org/XML/1998/namespace\"",
            "prefix = \"xml\"",
            "local-name = \"lang\"",
            "qualified-name = \"xml:lang\"",
            "value = \"en\"",
            "attribute-end = true",
            "child-count = 3",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/2000/svg\"",
            "local-name = \"svg\"",
            "attribute-count = 0",
            "child-count = 0",
            "template-contents = \"absent\"",
            "node-end = \"element\"",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/1998/Math/MathML\"",
            "local-name = \"math\"",
            "attribute-count = 0",
            "child-count = 0",
            "template-contents = \"absent\"",
            "node-end = \"element\"",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/1999/xhtml\"",
            "local-name = \"template\"",
            "attribute-count = 0",
            "child-count = 0",
            "template-contents = \"present\"",
            "template-child-count = 1",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/1999/xhtml\"",
            "local-name = \"template\"",
            "attribute-count = 0",
            "child-count = 0",
            "template-contents = \"present\"",
            "template-child-count = 1",
            "node-begin = \"text\"",
            "data = \"nested\\ntext \"",
            "node-end = \"text\"",
            "node-end = \"element\"",
            "node-end = \"element\"",
            "template-contents = \"absent\"",
            "node-end = \"element\"",
            "node-end = \"document\"",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn mixed_namespaces_canonical_attributes_and_nested_templates_are_valid() {
        assert_eq!(
            validate_web_observable_dom_tree_v1(valid_mixed_document().as_bytes()),
            Ok(())
        );
    }

    #[test]
    fn comments_and_processing_instructions_are_valid_nodes() {
        let source = [
            "format = \"web-observable-dom-tree-v1\"",
            "root-count = 1",
            "node-begin = \"document\"",
            "child-count = 2",
            "node-begin = \"comment\"",
            "data = \"comment\"",
            "node-end = \"comment\"",
            "node-begin = \"processing-instruction\"",
            "target = \"xml-stylesheet\"",
            "data = \"href=style.css\"",
            "node-end = \"processing-instruction\"",
            "node-end = \"document\"",
            "",
        ]
        .join("\n");
        assert_eq!(
            validate_web_observable_dom_tree_v1(source.as_bytes()),
            Ok(())
        );
    }

    fn valid_xmlns_and_xlink_document() -> String {
        [
            "format = \"web-observable-dom-tree-v1\"",
            "root-count = 1",
            "node-begin = \"document\"",
            "child-count = 1",
            "node-begin = \"element\"",
            "namespace-uri = \"http://www.w3.org/1999/xhtml\"",
            "local-name = \"html\"",
            "attribute-count = 2",
            "attribute-begin = true",
            "namespace-uri = \"http://www.w3.org/1999/xlink\"",
            "prefix = \"xlink\"",
            "local-name = \"href\"",
            "qualified-name = \"xlink:href\"",
            "value = \"target\"",
            "attribute-end = true",
            "attribute-begin = true",
            "namespace-uri = \"http://www.w3.org/2000/xmlns/\"",
            "prefix = \"xmlns\"",
            "local-name = \"xlink\"",
            "qualified-name = \"xmlns:xlink\"",
            "value = \"http://www.w3.org/1999/xlink\"",
            "attribute-end = true",
            "child-count = 0",
            "template-contents = \"absent\"",
            "node-end = \"element\"",
            "node-end = \"document\"",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn xmlns_and_xlink_attributes_obey_the_frozen_namespace_forms() {
        let source = valid_xmlns_and_xlink_document();
        assert_eq!(
            validate_web_observable_dom_tree_v1(source.as_bytes()),
            Ok(())
        );

        let invalid_xmlns = source.replacen(
            "prefix = \"xmlns\"\nlocal-name = \"xlink\"\nqualified-name = \"xmlns:xlink\"",
            "prefix = null\nlocal-name = \"xlink\"\nqualified-name = \"xlink\"",
            1,
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(invalid_xmlns.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidNamespace)
        );

        let invalid_xlink_name = source.replacen(
            "qualified-name = \"xlink:href\"",
            "qualified-name = \"href\"",
            1,
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(invalid_xlink_name.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidName)
        );
    }

    #[test]
    fn structure_counts_and_template_ownership_fail_closed() {
        let source = valid_mixed_document();
        for malformed in [
            source.replacen("child-count = 2", "child-count = 1", 1),
            source.replacen("local-name = \"template\"", "local-name = \"div\"", 1),
            source.replacen("node-end = \"text\"", "node-end = \"comment\"", 1),
            source.replacen("root-count = 1", "root-count = 2", 1),
        ] {
            assert!(validate_web_observable_dom_tree_v1(malformed.as_bytes()).is_err());
        }
    }

    #[test]
    fn unknown_node_kind_and_incorrect_attribute_count_fail_closed() {
        let unknown = [
            "format = \"web-observable-dom-tree-v1\"",
            "root-count = 1",
            "node-begin = \"document\"",
            "child-count = 1",
            "node-begin = \"future-node\"",
            "node-end = \"future-node\"",
            "node-end = \"document\"",
            "",
        ]
        .join("\n");
        assert_eq!(
            validate_web_observable_dom_tree_v1(unknown.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidStructure)
        );

        let wrong_count = valid_xmlns_and_xlink_document().replacen(
            "attribute-count = 2",
            "attribute-count = 3",
            1,
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(wrong_count.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidField)
        );
    }

    #[test]
    fn attribute_namespace_name_order_and_duplicates_fail_closed() {
        let source = valid_mixed_document();
        let wrong_prefix = source.replacen("prefix = \"xml\"", "prefix = \"xlink\"", 1);
        assert_eq!(
            validate_web_observable_dom_tree_v1(wrong_prefix.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidNamespace)
        );
        let wrong_qualified = source.replacen(
            "qualified-name = \"xml:lang\"",
            "qualified-name = \"lang\"",
            1,
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(wrong_qualified.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidName)
        );
        let unsupported_namespace = source.replacen(
            "namespace-uri = \"http://www.w3.org/2000/svg\"",
            "namespace-uri = \"urn:unsupported\"",
            1,
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(unsupported_namespace.as_bytes()),
            Err(ExternalArtifactValidationError::InvalidNamespace)
        );

        let first_attribute = "attribute-begin = true\nnamespace-uri = null\nprefix = null\nlocal-name = \"id\"\nqualified-name = \"id\"\nvalue = \"root\"\nattribute-end = true";
        let second_attribute = "attribute-begin = true\nnamespace-uri = \"http://www.w3.org/XML/1998/namespace\"\nprefix = \"xml\"\nlocal-name = \"lang\"\nqualified-name = \"xml:lang\"\nvalue = \"en\"\nattribute-end = true";
        let reversed = source.replace(
            &format!("{first_attribute}\n{second_attribute}"),
            &format!("{second_attribute}\n{first_attribute}"),
        );
        assert_eq!(
            validate_web_observable_dom_tree_v1(reversed.as_bytes()),
            Err(ExternalArtifactValidationError::NonCanonicalAttributeOrder)
        );
        let duplicate = source.replace(second_attribute, first_attribute);
        assert_eq!(
            validate_web_observable_dom_tree_v1(duplicate.as_bytes()),
            Err(ExternalArtifactValidationError::DuplicateAttribute)
        );
    }

    #[test]
    fn canonical_attribute_identity_uses_all_four_key_components() {
        let base = AttributeKey {
            namespace: Some(XML_NAMESPACE.to_owned()),
            local_name: "lang".to_owned(),
            prefix: Some("xml".to_owned()),
            qualified_name: "xml:lang".to_owned(),
        };
        let exact_duplicate = base.clone();
        assert_eq!(
            compare_attribute_key(&base, &exact_duplicate),
            Ordering::Equal
        );
        assert_eq!(
            validate_attribute_progression(&base, &exact_duplicate),
            Err(ExternalArtifactValidationError::DuplicateAttribute)
        );

        // V1 identity includes prefix and qualified name. This unit-level key
        // proof deliberately does not claim that every tuple is a valid XML
        // namespace/name combination accepted by parse_attribute.
        let later_component_difference = AttributeKey {
            prefix: Some("xmlz".to_owned()),
            qualified_name: "xmlz:lang".to_owned(),
            ..base.clone()
        };
        assert_eq!(
            compare_attribute_key(&base, &later_component_difference),
            Ordering::Less
        );
        assert_eq!(
            validate_attribute_progression(&base, &later_component_difference),
            Ok(())
        );
        assert_eq!(
            compare_attribute_key(&later_component_difference, &base),
            Ordering::Greater
        );
        assert_eq!(
            validate_attribute_progression(&later_component_difference, &base),
            Err(ExternalArtifactValidationError::NonCanonicalAttributeOrder)
        );
    }

    #[test]
    fn strings_counts_headers_and_complete_consumption_are_canonical() {
        let source = valid_mixed_document();
        for malformed in [
            source.replacen("data = \"nested\\ntext", "data = \"nested\\u000atext", 1),
            source.replacen("child-count = 2", "child-count = 02", 1),
            source.replacen(
                "format = \"web-observable-dom-tree-v1\"",
                "format = \"other\"",
                1,
            ),
            source.trim_end_matches('\n').to_owned(),
            format!("{source}\n"),
        ] {
            assert!(validate_web_observable_dom_tree_v1(malformed.as_bytes()).is_err());
        }
        let invalid_utf8 = [EMPTY, &[0xff, b'\n']].concat();
        assert_eq!(
            validate_web_observable_dom_tree_v1(&invalid_utf8),
            Err(ExternalArtifactValidationError::InvalidUtf8)
        );
    }

    #[test]
    fn declared_maximum_is_inclusive() {
        let mut at_limit = EMPTY.to_vec();
        at_limit.resize(MAX_WEB_OBSERVABLE_DOM_TREE_BYTES_V1 as usize, b'x');
        assert_ne!(
            validate_web_observable_dom_tree_v1(&at_limit),
            Err(ExternalArtifactValidationError::TooLarge)
        );
        at_limit.push(b'x');
        assert_eq!(
            validate_web_observable_dom_tree_v1(&at_limit),
            Err(ExternalArtifactValidationError::TooLarge)
        );
    }

    #[test]
    fn stack_and_string_reservation_failures_are_deterministic() {
        for site in [
            ReservationSite::ComparableDomStack,
            ReservationSite::ComparableDomString,
        ] {
            assert_eq!(
                validate_web_observable_dom_tree_v1_with_policy(
                    EMPTY,
                    &mut RejectReservationAt::new(site),
                ),
                Err(ExternalArtifactValidationError::Allocation)
            );
        }
    }
}
