use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryWire {
    pub format: String,
    pub captures: Vec<CaptureWire>,
    pub attachments: Vec<AttachmentWire>,
    pub advisory_tracks: Vec<TrackWire>,
    pub baseline_notes: Vec<NoteWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CaptureWire {
    pub capture_id: String,
    pub artifact_path: String,
    pub provenance_format: String,
    pub engine_product: String,
    pub engine_version: String,
    pub engine_build_revision: Option<String>,
    pub platform_os_family: String,
    pub platform_os_version: String,
    pub architecture: String,
    pub viewport: ViewportWire,
    pub device_scale: DeviceScaleWire,
    pub controlled_fonts: FontsWire,
    pub resource_network_policy: String,
    pub pinned_resources: Vec<ResourceWire>,
    pub fixture_source_project: String,
    pub fixture_immutable_revision: String,
    pub fixture_content_sha256: String,
    pub capture_mechanism: String,
    pub capture_mechanism_version: String,
    pub capture_algorithm: String,
    pub capture_algorithm_version: String,
    pub capture_algorithm_source_sha256: String,
    pub capture_configuration_sha256: String,
    pub invocation_arguments: Vec<String>,
    pub artifact_format: String,
    pub artifact_utf8_byte_length: u64,
    pub artifact_sha256: String,
    pub target_parser_input_context: String,
    pub collection_policy: String,
    pub collection_policy_version: String,
}

#[derive(Deserialize)]
#[serde(tag = "applicability", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum ViewportWire {
    Applicable {
        width_css_px: u32,
        height_css_px: u32,
    },
    NotApplicable {
        reason: String,
    },
}

#[derive(Deserialize)]
#[serde(tag = "applicability", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum DeviceScaleWire {
    Applicable { numerator: u32, denominator: u32 },
    NotApplicable { reason: String },
}

#[derive(Deserialize)]
#[serde(tag = "applicability", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum FontsWire {
    Applicable { items: Vec<FontWire> },
    NotApplicable { reason: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FontWire {
    pub family: String,
    pub face_style: String,
    pub version: String,
    pub file_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourceWire {
    pub identity: String,
    pub content_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrackWire {
    pub track_id: String,
    pub engine_product: String,
    pub platform_os_family: String,
    pub architecture: String,
    pub comparable_observation_surface: String,
    pub capture_algorithm: String,
    pub capture_algorithm_version: String,
    pub target_parser_input_context: String,
    pub collection_policy: String,
    pub collection_policy_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionVariantWire {
    pub kind: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttachmentWire {
    pub test_id: String,
    pub observation_surface: String,
    pub execution_variant: ExecutionVariantWire,
    pub comparable_observation_surface: String,
    pub track_id: String,
    pub capture_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NoteWire {
    pub note_id: String,
    pub test_id: String,
    pub observation_surface: String,
    pub execution_variant: ExecutionVariantWire,
    pub comparable_observation_surface: String,
    pub text: String,
    pub capture_id: Option<String>,
}
