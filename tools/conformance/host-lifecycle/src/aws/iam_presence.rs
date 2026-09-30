//! GetInstanceProfile-only structural presence. Never decodes AWS values or issues requests.
use super::{
    identity_observation::{NormalizedIdentityV3, not_returned, profile_members, role_members},
    response_limits::ObservationRound,
};
use crate::{
    Error, Result,
    provider::{
        limits::RESPONSE_BYTES, management_observation_v2::ObservationValueV2,
        observation_v3::ObservationDataV3,
    },
    require,
};
use aws_sdk_iam::operation::get_instance_profile::{
    GetInstanceProfileInput, GetInstanceProfileOutput,
};
use aws_smithy_runtime_api::client::{
    interceptors::{
        Intercept,
        context::{
            AfterDeserializationInterceptorContextRef, BeforeDeserializationInterceptorContextRef,
            BeforeSerializationInterceptorContextRef, FinalizerInterceptorContextRef,
        },
    },
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::config_bag::ConfigBag;
use std::sync::{Arc, Mutex};
use xmlparser::{ElementEnd, Token};

#[derive(Default)]
struct RolePresence {
    arn: bool,
    id: bool,
}
#[derive(Default)]
struct ProfilePresence {
    profile: bool,
    arn: bool,
    id: bool,
    roles_present: bool,
    roles: Vec<RolePresence>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Node {
    Root,
    Result,
    Profile,
    Roles,
    Role(usize),
    Scalar,
    Other,
}
struct Frame<'a> {
    prefix: &'a str,
    local: &'a str,
    node: Node,
    text_seen: bool,
}
fn once(flag: &mut bool) -> Result<()> {
    require(!*flag, "ambiguous IAM singleton")?;
    *flag = true;
    Ok(())
}

/// Private fixed-path scanner. Borrowed tokenizer spans never escape this function.
fn scan(bytes: &[u8], round: &ObservationRound) -> Result<ProfilePresence> {
    require(
        bytes.len() as u64 <= RESPONSE_BYTES,
        "IAM presence response bound",
    )?;
    round.remaining()?;
    let xml = std::str::from_utf8(bytes).map_err(|_| Error("IAM presence UTF-8"))?;
    let mut output = ProfilePresence::default();
    let mut stack: Vec<Frame<'_>> = Vec::new();
    let mut attributes = Vec::new();
    let mut root_seen = false;
    let mut result_seen = false;
    let mut first_root_child = true;
    let mut opening = false;
    for (index, token) in xmlparser::Tokenizer::from(xml).enumerate() {
        if index % 64 == 0 {
            round.remaining()?;
        }
        match token.map_err(|_| Error("IAM presence XML"))? {
            Token::ElementStart { prefix, local, .. } => {
                require(!opening && stack.len() < 128, "IAM presence nesting")?;
                if stack.last().is_some_and(|f| f.node == Node::Scalar) {
                    return Err(Error("IAM scalar structure unsupported"));
                }
                let node = match stack.last().map(|f| f.node) {
                    None => {
                        require(
                            !root_seen && local.as_str() == "GetInstanceProfileResponse",
                            "IAM response root",
                        )?;
                        root_seen = true;
                        Node::Root
                    }
                    Some(Node::Root) => {
                        if first_root_child {
                            require(
                                local.as_str() == "GetInstanceProfileResult",
                                "IAM result position",
                            )?;
                            first_root_child = false;
                        }
                        if local.as_str() == "GetInstanceProfileResult" {
                            once(&mut result_seen)?;
                            Node::Result
                        } else {
                            Node::Other
                        }
                    }
                    Some(Node::Result) if local.as_str() == "InstanceProfile" => {
                        once(&mut output.profile)?;
                        Node::Profile
                    }
                    Some(Node::Profile) => match local.as_str() {
                        "Arn" => {
                            once(&mut output.arn)?;
                            Node::Scalar
                        }
                        "InstanceProfileId" => {
                            once(&mut output.id)?;
                            Node::Scalar
                        }
                        "Roles" => {
                            once(&mut output.roles_present)?;
                            Node::Roles
                        }
                        _ => Node::Other,
                    },
                    Some(Node::Roles) if local.as_str() == "member" => {
                        require(output.roles.len() < 128, "IAM role occurrence bound")?;
                        let i = output.roles.len();
                        output.roles.push(RolePresence::default());
                        Node::Role(i)
                    }
                    Some(Node::Role(i)) => match local.as_str() {
                        "Arn" => {
                            once(&mut output.roles[i].arn)?;
                            Node::Scalar
                        }
                        "RoleId" => {
                            once(&mut output.roles[i].id)?;
                            Node::Scalar
                        }
                        _ => Node::Other,
                    },
                    _ => Node::Other,
                };
                stack.push(Frame {
                    prefix: prefix.as_str(),
                    local: local.as_str(),
                    node,
                    text_seen: false,
                });
                attributes.clear();
                opening = true;
            }
            Token::Attribute {
                prefix,
                local,
                value,
                ..
            } => {
                // Smithy's next_start_element turns attribute-unescape errors into None.
                // Exclude all references (even valid ones) on every element before decoding.
                // This is a structural syntax restriction, not attribute-value decoding.
                require(
                    !value.as_str().contains('&'),
                    "IAM XML attribute references unsupported",
                )?;
                require(opening && attributes.len() < 128, "IAM XML attribute bound")?;
                let name = (prefix.as_str(), local.as_str());
                require(!attributes.contains(&name), "duplicate IAM XML attribute")?;
                attributes.push(name);
                // Namespace declarations are structural. No xsi:nil or other scalar conventions.
                if stack.last().is_some_and(|f| f.node != Node::Other) {
                    require(
                        prefix.as_str() == "xmlns"
                            || (prefix.as_str().is_empty() && local.as_str() == "xmlns"),
                        "IAM XML attribute unsupported",
                    )?;
                }
            }
            Token::ElementEnd { end, .. } => match end {
                ElementEnd::Open => {
                    require(opening, "IAM XML opening")?;
                    opening = false;
                }
                ElementEnd::Empty => {
                    require(opening, "IAM XML empty element")?;
                    stack.pop().ok_or(Error("IAM XML nesting"))?;
                    opening = false;
                }
                ElementEnd::Close(prefix, local) => {
                    require(!opening, "IAM XML close")?;
                    let frame = stack.pop().ok_or(Error("IAM XML nesting"))?;
                    require(
                        frame.prefix == prefix.as_str() && frame.local == local.as_str(),
                        "IAM XML mismatched close",
                    )?;
                }
            },
            Token::Text { text } => {
                if let Some(frame) = stack.last_mut() {
                    if frame.node == Node::Scalar {
                        once(&mut frame.text_seen)
                            .map_err(|_| Error("fragmented IAM scalar unsupported"))?;
                    } else if frame.node != Node::Other {
                        // Structural whitespace only; the inspector does not interpret values.
                        require(
                            text.as_str()
                                .bytes()
                                .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')),
                            "IAM container text unsupported",
                        )?;
                    }
                }
            }
            Token::Cdata { .. } if stack.last().is_some_and(|f| f.node != Node::Other) => {
                return Err(Error("IAM monitored CDATA unsupported"));
            }
            Token::DtdStart { .. }
            | Token::EmptyDtd { .. }
            | Token::EntityDeclaration { .. }
            | Token::DtdEnd { .. } => return Err(Error("IAM DTD unsupported")),
            Token::Declaration {
                encoding: Some(encoding),
                ..
            } => require(
                encoding.as_str().eq_ignore_ascii_case("utf-8"),
                "IAM XML encoding unsupported",
            )?,
            _ => (),
        }
    }
    require(
        root_seen && result_seen && stack.is_empty() && !opening,
        "incomplete IAM XML",
    )?;
    round.remaining()?;
    Ok(output)
}

// Private to this module: no API can pair an independently supplied output and presence map.
fn normalize(
    output: &GetInstanceProfileOutput,
    presence: ProfilePresence,
) -> Result<NormalizedIdentityV3> {
    let value = output
        .instance_profile()
        .ok_or(Error("IAM profile correlation"))?;
    fn correlated(present: bool, value: &str) -> Result<Option<&str>> {
        require(
            present || value.is_empty(),
            "IAM identifier presence correlation",
        )?;
        Ok(present.then_some(value))
    }
    let profile = if !presence.profile {
        require(
            value.arn().is_empty()
                && value.instance_profile_id().is_empty()
                && value.roles().is_empty(),
            "IAM default profile correlation",
        )?;
        not_returned()
    } else {
        require(
            presence.roles.len() == value.roles().len(),
            "IAM role cardinality correlation",
        )?;
        let roles = if !presence.roles_present {
            require(value.roles().is_empty(), "IAM missing roles correlation")?;
            not_returned()
        } else {
            let mut roles = Vec::with_capacity(presence.roles.len());
            for (role, flags) in value.roles().iter().zip(presence.roles) {
                roles.push(role_members(
                    correlated(flags.arn, role.arn())?,
                    correlated(flags.id, role.role_id())?,
                ));
            }
            ObservationValueV2::Present(roles.try_into()?)
        };
        ObservationValueV2::Present(profile_members(
            correlated(presence.arn, value.arn())?,
            correlated(presence.id, value.instance_profile_id())?,
            roles,
        ))
    };
    NormalizedIdentityV3::new(ObservationDataV3::Profile { profile })
}

enum State {
    Fresh,
    Started,
    Captured(ProfilePresence),
    Normalized(NormalizedIdentityV3),
    Complete(NormalizedIdentityV3),
    Failed,
}
pub(super) struct IamProfilePresenceInterceptor {
    state: Arc<Mutex<State>>,
    round: ObservationRound,
}
pub(super) struct IamProfileObservationReceiver {
    state: Arc<Mutex<State>>,
    round: ObservationRound,
}
impl std::fmt::Debug for IamProfilePresenceInterceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IamProfilePresenceInterceptor")
    }
}
pub(super) fn capture_instance_profile(
    round: ObservationRound,
) -> (IamProfilePresenceInterceptor, IamProfileObservationReceiver) {
    let state = Arc::new(Mutex::new(State::Fresh));
    (
        IamProfilePresenceInterceptor {
            state: state.clone(),
            round: round.clone(),
        },
        IamProfileObservationReceiver { state, round },
    )
}
impl IamProfileObservationReceiver {
    /// Consume only after the corresponding SDK send succeeds. No SDK output argument exists.
    pub(super) fn take(self) -> Result<NormalizedIdentityV3> {
        self.round.remaining()?;
        match std::mem::replace(
            &mut *self.state.lock().map_err(|_| Error("IAM capture state"))?,
            State::Failed,
        ) {
            State::Complete(value) => Ok(value),
            _ => Err(Error("IAM capture unavailable")),
        }
    }
}
impl IamProfilePresenceInterceptor {
    fn transition(&self, f: impl FnOnce(State) -> Result<State>) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| Error("IAM capture state"))?;
        let previous = std::mem::replace(&mut *state, State::Failed);
        self.round.remaining()?;
        *state = f(previous)?;
        Ok(())
    }
}
type HookResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;
impl Intercept for IamProfilePresenceInterceptor {
    fn name(&self) -> &'static str {
        "IamProfilePresence"
    }
    fn read_before_execution(
        &self,
        ctx: &BeforeSerializationInterceptorContextRef<'_>,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            require(
                matches!(state, State::Fresh)
                    && ctx
                        .input()
                        .downcast_ref::<GetInstanceProfileInput>()
                        .is_some(),
                "IAM capture operation/reuse",
            )?;
            Ok(State::Started)
        })?;
        Ok(())
    }
    fn read_before_deserialization(
        &self,
        ctx: &BeforeDeserializationInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            require(
                matches!(state, State::Started),
                "IAM capture repeated response",
            )?;
            if !ctx.response().status().is_success() {
                return Ok(State::Failed);
            }
            let bytes = ctx
                .response()
                .body()
                .bytes()
                .ok_or(Error("IAM capture requires bounded completed body"))?;
            Ok(State::Captured(scan(bytes, &self.round)?))
        })?;
        Ok(())
    }
    fn read_after_deserialization(
        &self,
        ctx: &AfterDeserializationInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            let Ok(output) = ctx.output_or_error() else {
                return Ok(State::Failed);
            };
            let State::Captured(presence) = state else {
                return Err(Error("IAM presence unavailable"));
            };
            let output = output
                .downcast_ref::<GetInstanceProfileOutput>()
                .ok_or(Error("IAM output correlation"))?;
            Ok(State::Normalized(normalize(output, presence)?))
        })?;
        Ok(())
    }
    fn read_after_execution(
        &self,
        ctx: &FinalizerInterceptorContextRef<'_>,
        _: &RuntimeComponents,
        _: &mut ConfigBag,
    ) -> HookResult {
        self.transition(|state| {
            if !matches!(ctx.output_or_error(), Some(Ok(_))) {
                return Ok(State::Failed);
            }
            match state {
                State::Normalized(value) => Ok(State::Complete(value)),
                _ => Err(Error("IAM capture completion")),
            }
        })?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "iam_presence_tests.rs"]
mod tests;
