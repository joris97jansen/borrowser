//! Sequential, fixed-depth policy projection. Duplicate keys are never collected into a map.
use super::ec2_observation::{Normalizer, ReadResult, member, member_bound};
use crate::{
    canonical,
    provider::{
        coverage::ReadFailureV1,
        ec2_observation_v4::FactsV4,
        endpoint_policy_observation_v4::*,
        limits::{LimitKind, RECORDS},
        observation::EvidenceList,
    },
};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::fmt;

// RawValue is a borrowed syntax slice, never a retained AST. Walking each bounded
// container avoids f64 conversion of unsupported numeric literals such as 1e400.
fn work(raw: &RawValue, n: &Normalizer<'_>, count: &mut u64, depth: usize) -> ReadResult<()> {
    n.check()?;
    *count += 1;
    if *count > RECORDS || depth >= 128 {
        return Err(ReadFailureV1::Limit(LimitKind::Records));
    }
    let mut scanner = Work {
        n,
        count,
        depth,
        failure: None,
    };
    let mut decoder = serde_json::Deserializer::from_str(raw.get());
    let result = match shape(raw) {
        PolicyShapeV4::Object => decoder.deserialize_map(&mut scanner),
        PolicyShapeV4::Array => decoder.deserialize_seq(&mut scanner),
        // RawValue's skip parser accepts lone UTF-16 surrogates. Validate
        // actual strings before any later infallible structural projection.
        PolicyShapeV4::String => {
            return serde_json::from_str::<String>(raw.get())
                .map(|_| ())
                .map_err(|_| ReadFailureV1::Malformed);
        }
        _ => return Ok(()),
    };
    result.map_err(|_| scanner.failure.unwrap_or(ReadFailureV1::Malformed))
}
// Count as each occurrence is inspected, even if a later container member
// overflows. No temporary unbounded collection or f64 decoding precedes this walk.
struct Work<'a, 'n> {
    n: &'a Normalizer<'n>,
    count: &'a mut u64,
    depth: usize,
    failure: Option<ReadFailureV1>,
}
impl Work<'_, '_> {
    fn fail<E: serde::de::Error>(&mut self, reason: ReadFailureV1) -> E {
        self.failure = Some(reason);
        E::custom("bounded policy traversal")
    }
    fn child<E: serde::de::Error>(&mut self, raw: &RawValue) -> std::result::Result<(), E> {
        work(raw, self.n, self.count, self.depth + 1).map_err(|reason| self.fail(reason))
    }
}
impl<'de> Visitor<'de> for &mut Work<'_, '_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded policy container")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> std::result::Result<(), A::Error> {
        let mut length = 0;
        while let Some(key) = map.next_key::<String>()? {
            *self.count += 1;
            if length == 128 {
                return Err(self.fail(ReadFailureV1::Limit(LimitKind::Records)));
            }
            if key.len() > 2048 {
                return Err(self.fail(ReadFailureV1::Limit(LimitKind::RecordBytes)));
            }
            let value = map.next_value::<&RawValue>()?;
            self.child(value)?;
            length += 1;
        }
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
        let mut length = 0;
        while let Some(value) = seq.next_element::<&RawValue>()? {
            if length == 128 {
                *self.count += 1;
                return Err(self.fail(ReadFailureV1::Limit(LimitKind::Records)));
            }
            self.child(value)?;
            length += 1;
        }
        Ok(())
    }
}

struct Members<'a>(Vec<(String, &'a RawValue)>);
impl<'de> Deserialize<'de> for Members<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Members<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("bounded policy members")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut items = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    if items.len() == 128 {
                        return Err(serde::de::Error::custom("policy member bound"));
                    }
                    items.push((key, map.next_value()?));
                }
                Ok(Members(items))
            }
        }
        d.deserialize_map(V)
    }
}
fn members(raw: &RawValue) -> ReadResult<Vec<(String, &RawValue)>> {
    serde_json::from_str::<Members<'_>>(raw.get())
        .map(|v| v.0)
        .map_err(|_| ReadFailureV1::Limit(LimitKind::Records))
}
fn array(raw: &RawValue) -> ReadResult<EvidenceList<&RawValue>> {
    serde_json::from_str(raw.get()).map_err(|_| ReadFailureV1::Limit(LimitKind::Records))
}
fn shape(raw: &RawValue) -> PolicyShapeV4 {
    match raw.get().as_bytes()[0] {
        b'{' => PolicyShapeV4::Object,
        b'[' => PolicyShapeV4::Array,
        b'"' => PolicyShapeV4::String,
        b'n' => PolicyShapeV4::Null,
        b't' | b'f' => PolicyShapeV4::Boolean,
        _ => PolicyShapeV4::Number,
    }
}
fn text(raw: &RawValue, bound: usize) -> PolicyTextV4 {
    if shape(raw) == PolicyShapeV4::String {
        let s: String = serde_json::from_str(raw.get()).expect("validated JSON string");
        PolicyTextV4::Text(member_bound(Some(&s), bound))
    } else {
        PolicyTextV4::Unsupported(shape(raw))
    }
}
fn strings(raw: &RawValue) -> ReadResult<PolicyStringsV4> {
    Ok(match shape(raw) {
        PolicyShapeV4::String => PolicyStringsV4::Scalar(text(raw, 2048)),
        PolicyShapeV4::Array => PolicyStringsV4::Array(
            array(raw)?
                .as_slice()
                .iter()
                .map(|r| text(r, 2048))
                .collect::<Vec<_>>()
                .try_into()
                .expect("bounded array"),
        ),
        s => PolicyStringsV4::Scalar(PolicyTextV4::Unsupported(s)),
    })
}
fn literal(raw: &RawValue) -> PolicyLiteralV4 {
    match shape(raw) {
        PolicyShapeV4::String => {
            let PolicyTextV4::Text(v) = text(raw, 2048) else {
                unreachable!()
            };
            PolicyLiteralV4::Text(v)
        }
        PolicyShapeV4::Boolean => PolicyLiteralV4::Boolean(raw.get() == "true"),
        s => PolicyLiteralV4::Unsupported(s),
    }
}
fn values(raw: &RawValue) -> ReadResult<PolicyValuesV4> {
    Ok(if shape(raw) == PolicyShapeV4::Array {
        PolicyValuesV4::Array(
            array(raw)?
                .as_slice()
                .iter()
                .map(|r| literal(r))
                .collect::<Vec<_>>()
                .try_into()
                .expect("bounded array"),
        )
    } else {
        PolicyValuesV4::Scalar(literal(raw))
    })
}
fn principals(raw: &RawValue) -> ReadResult<PolicyPrincipalsV4> {
    Ok(match shape(raw) {
        PolicyShapeV4::String => PolicyPrincipalsV4::Literal(text(raw, 2048)),
        PolicyShapeV4::Object => {
            let mut result = Vec::new();
            for (key, v) in members(raw)? {
                result.push(PolicyPrincipalEntryV4 {
                    principal: member(Some(&key)),
                    identities: strings(v)?,
                });
            }
            PolicyPrincipalsV4::Entries(result.try_into().expect("bounded object"))
        }
        s => PolicyPrincipalsV4::Unsupported(s),
    })
}
fn conditions(raw: &RawValue) -> ReadResult<PolicyConditionsV4> {
    if shape(raw) != PolicyShapeV4::Object {
        return Ok(PolicyConditionsV4::Unsupported(shape(raw)));
    }
    let mut operators = Vec::new();
    for (op, raw) in members(raw)? {
        let entries = if shape(raw) == PolicyShapeV4::Object {
            let mut entries = Vec::new();
            for (key, v) in members(raw)? {
                entries.push(PolicyConditionV4 {
                    key: member(Some(&key)),
                    values: values(v)?,
                });
            }
            PolicyConditionEntriesV4::Entries(entries.try_into().expect("bounded object"))
        } else {
            PolicyConditionEntriesV4::Unsupported(shape(raw))
        };
        operators.push(PolicyOperatorV4 {
            operator: member(Some(&op)),
            conditions: entries,
        });
    }
    Ok(PolicyConditionsV4::Operators(
        operators.try_into().expect("bounded object"),
    ))
}
fn unsupported(name: String, raw: &RawValue) -> ReadResult<UnsupportedPolicyMemberV4> {
    Ok(UnsupportedPolicyMemberV4 {
        name: member(Some(&name)),
        shape: shape(raw),
    })
}
fn statement(raw: &RawValue) -> ReadResult<PolicyStatementV4> {
    let mut result = Vec::new();
    for (key, raw) in members(raw)? {
        use PolicyStatementMemberV4 as M;
        result.push(match key.as_str() {
            "Sid" => M::Sid(text(raw, 128)),
            "Effect" => M::Effect(text(raw, 2048)),
            "Principal" => M::Principal(principals(raw)?),
            "NotPrincipal" => M::NotPrincipal(principals(raw)?),
            "Action" => M::Action(strings(raw)?),
            "NotAction" => M::NotAction(strings(raw)?),
            "Resource" => M::Resource(strings(raw)?),
            "NotResource" => M::NotResource(strings(raw)?),
            "Condition" => M::Condition(conditions(raw)?),
            _ => M::Unsupported(unsupported(key, raw)?),
        });
    }
    Ok(PolicyStatementV4 {
        members: result.try_into().expect("bounded members"),
    })
}
fn statements(raw: &RawValue) -> ReadResult<PolicyStatementsV4> {
    Ok(match shape(raw) {
        PolicyShapeV4::Object => PolicyStatementsV4::Object(statement(raw)?),
        PolicyShapeV4::Array => {
            let mut result = Vec::new();
            for raw in array(raw)?.as_slice() {
                result.push(if shape(raw) == PolicyShapeV4::Object {
                    PolicyStatementValueV4::Object(statement(raw)?)
                } else {
                    PolicyStatementValueV4::Unsupported(shape(raw))
                });
            }
            PolicyStatementsV4::Array(result.try_into().expect("bounded array"))
        }
        s => PolicyStatementsV4::Unsupported(s),
    })
}
pub(super) fn parse(
    input: Option<&str>,
    n: &mut Normalizer<'_>,
) -> ReadResult<EndpointPolicyValueV4> {
    let Some(input) = input else {
        return Ok(EndpointPolicyValueV4::NotReturned);
    };
    if input.is_empty() {
        return Ok(EndpointPolicyValueV4::Empty);
    }
    let raw: &RawValue = match serde_json::from_str(input) {
        Ok(raw) => raw,
        Err(_) => return Ok(EndpointPolicyValueV4::Malformed),
    };
    let mut count = 0;
    let scanned = work(raw, n, &mut count, 0);
    n.policy_occurrences += count;
    match scanned {
        Ok(()) => (),
        Err(ReadFailureV1::Malformed) => return Ok(EndpointPolicyValueV4::Malformed),
        Err(reason) => return Err(reason),
    }
    if shape(raw) != PolicyShapeV4::Object {
        return Ok(EndpointPolicyValueV4::Unsupported(shape(raw)));
    }
    let mut result = Vec::new();
    for (key, raw) in members(raw)? {
        n.check()?;
        result.push(match key.as_str() {
            "Version" => PolicyDocumentMemberV4::Version(text(raw, 2048)),
            "Id" => PolicyDocumentMemberV4::Id(text(raw, 2048)),
            "Statement" => PolicyDocumentMemberV4::Statement(statements(raw)?),
            _ => PolicyDocumentMemberV4::Unsupported(unsupported(key, raw)?),
        });
    }
    let doc = PolicyDocumentV4 {
        members: result.try_into().expect("bounded members"),
    };
    check_policy_bytes(&doc)?;
    doc.facts()
        .map_err(|_| ReadFailureV1::Limit(LimitKind::Records))?;
    Ok(EndpointPolicyValueV4::Document(doc))
}

fn check_policy_bytes(doc: &PolicyDocumentV4) -> ReadResult<()> {
    // This boundary accepts only the closed, fixed-depth V4 projection: derived
    // serialization produces arrays, static ASCII schema keys, strings and bools.
    // Returned names are string values, never schema keys; there are no numeric
    // payloads, recursive variants or fallible custom serializers. Its depth fits
    // the frozen encoder even inside a record. Thus the only reachable encoding
    // error here is its document byte ceiling, which also exceeds the policy's
    // smaller ceiling. This mapping must not be applied to raw JSON or validators.
    let bytes = canonical::encode(doc).map_err(|_| ReadFailureV1::Limit(LimitKind::RecordBytes))?;
    if bytes.len() > 8192 {
        return Err(ReadFailureV1::Limit(LimitKind::RecordBytes));
    }
    Ok(())
}
