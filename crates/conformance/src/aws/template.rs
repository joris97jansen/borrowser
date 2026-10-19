//! One private JSON representation for this asset-free CDK template and CLI envelopes.
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub(super) const MAX_JSON: usize = 256 * 1024;

struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Json;
        impl<'de> Visitor<'de> for Json {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("integer-only JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut items = vec![];
                while let Some(Strict(v)) = a.next_element()? {
                    items.push(v);
                }
                Ok(Strict(Value::Array(items)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Strict, A::Error> {
                let mut items = BTreeMap::new();
                while let Some((key, Strict(v))) = a.next_entry::<String, Strict>()? {
                    if items.insert(key, v).is_some() {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                }
                Ok(Strict(Value::Object(items.into_iter().collect())))
            }
        }
        d.deserialize_any(Json)
    }
}

pub(super) fn json(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_JSON {
        return Err("JSON exceeds response limit".into());
    }
    let Strict(v) = serde_json::from_slice(bytes).map_err(|e| format!("invalid JSON: {e}"))?;
    if !v.is_object() {
        return Err("expected JSON object".into());
    }
    Ok(v)
}
pub(super) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn digest(v: &Value) -> Result<String, String> {
    // Reparse even internally constructed values so every caller has the same domain.
    let strict = json(&serde_json::to_vec(v).map_err(|e| e.to_string())?)?;
    let mut bytes = b"borrowser-ag4-template-v1\n".to_vec();
    serde_json::to_writer(&mut bytes, &strict).map_err(|e| e.to_string())?;
    Ok(sha256(&bytes))
}
pub(super) fn original(response: &[u8]) -> Result<Value, String> {
    let envelope = json(response)?;
    match envelope.get("TemplateBody") {
        Some(Value::String(s)) => json(s.as_bytes()),
        Some(v @ Value::Object(_)) => Ok(v.clone()),
        _ => Err("missing or invalid original TemplateBody".into()),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding {
    pub run_id: String,
    pub account: String,
    pub region: String,
    pub stack_name: String,
    pub role_arn: String,
    pub tags: BTreeMap<String, String>,
    pub timeout_minutes: u32,
    pub disable_rollback: bool,
    pub termination_protection: bool,
}
pub(super) fn binding(v: &Value) -> Result<Binding, String> {
    if v.get("Parameters").is_some() || v.get("Transform").is_some() {
        return Err("parameters/transforms are outside AG4".into());
    }
    let b: Binding = serde_json::from_value(v["Metadata"]["BorrowserAg4Operation"].clone())
        .map_err(|e| e.to_string())?;
    if b.region != "eu-west-1"
        || b.timeout_minutes != 10
        || b.disable_rollback
        || b.termination_protection
        || b.stack_name != format!("borrowser-ag4-{}", b.run_id)
        || b.run_id.len() != 32
        || !b
            .run_id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        || b.account.len() != 12
        || !b.account.bytes().all(|c| c.is_ascii_digit())
        || !b
            .role_arn
            .starts_with(&format!("arn:aws:iam::{}:role/", b.account))
        || b.tags
            != BTreeMap::from([
                ("borrowser:run-id".into(), b.run_id.clone()),
                ("borrowser:purpose".into(), "ag4".into()),
            ])
    {
        return Err("invalid operation binding".into());
    }
    let resources = v["Resources"].as_object().ok_or("missing Resources")?;
    if resources.len() != 2
        || v["Resources"]["Runner"]["Type"] != "AWS::EC2::Instance"
        || v["Resources"]["RunnerSecurityGroup"]["Type"] != "AWS::EC2::SecurityGroup"
    {
        return Err("unexpected run-owned resources".into());
    }
    for r in resources.values() {
        if r["DeletionPolicy"] != "Delete" || r["UpdateReplacePolicy"] != "Delete" {
            return Err("non-deleting resource policy".into());
        }
    }
    let p = &resources["Runner"]["Properties"];
    if p["InstanceType"] != "c6i.large"
        || p["PropagateTagsToVolumeOnCreation"] != true
        || p["DisableApiTermination"] != false
        || p["BlockDeviceMappings"].as_array().map(Vec::len) != Some(1)
        || p["BlockDeviceMappings"][0]["Ebs"]["DeleteOnTermination"] != true
        || p["BlockDeviceMappings"][0]["Ebs"]["VolumeSize"] != 16
        || p["BlockDeviceMappings"][0]["Ebs"]["VolumeType"] != "gp3"
        || p["NetworkInterfaces"].as_array().map(Vec::len) != Some(1)
        || p["NetworkInterfaces"][0]["DeleteOnTermination"] != true
    {
        return Err("invalid instance deletion/resource limits".into());
    }
    let tags = p["Tags"].as_array().ok_or("missing instance tags")?;
    for (key, value) in [
        ("borrowser:run-id", serde_json::json!(b.run_id)),
        (
            "borrowser:stack-id",
            serde_json::json!({"Ref":"AWS::StackId"}),
        ),
        ("borrowser:purpose", serde_json::json!("ag4")),
    ] {
        if tags
            .iter()
            .filter(|t| t["Key"] == key && t["Value"] == value)
            .count()
            != 1
        {
            return Err("missing volume ownership tag propagation".into());
        }
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn representation_preserves_values_and_array_order() {
        let a = json(br#"{"b":[2,1],"a":{"x":"a"}}"#).unwrap();
        let b = json(br#"{ "a": {"x":"\u0061"}, "b": [2, 1] }"#).unwrap();
        assert_eq!(digest(&a), digest(&b));
        assert_ne!(
            digest(&a),
            digest(&json(br#"{"b":[1,2],"a":{"x":"a"}}"#).unwrap())
        );
        for bytes in [
            br#"{"a":1,"a":2}"#.as_slice(),
            br#"{"a":1.0}"#,
            br#"{"a":1e0}"#,
            br#"{"a":1} null"#,
            b"{",
        ] {
            assert!(json(bytes).is_err());
        }
    }
    #[test]
    fn original_is_complete_and_strict() {
        for bytes in [
            b"{}".as_slice(),
            br#"{"TemplateBody":null}"#,
            br#"{"TemplateBody":"{"}"#,
            br#"{"TemplateBody":{},"TemplateBody":{}}"#,
        ] {
            assert!(original(bytes).is_err());
        }
        assert_eq!(
            original(br#"{"TemplateBody":{"Resources":{}}}"#).unwrap(),
            original(br#"{"TemplateBody":"{\"Resources\":{}}"}"#).unwrap()
        );
    }
}
