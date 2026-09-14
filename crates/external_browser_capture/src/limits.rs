pub const CONFIG_BYTES: usize = 65_536;
pub const MANIFEST_BYTES: usize = 1_048_576;
pub const FIXTURE_BYTES: usize = 1_048_576;
pub const ARTIFACT_BYTES: usize = 8_388_608;
pub const MESSAGE_BYTES: usize = 16_777_216;
pub const PROTOCOL_BYTES: usize = 67_108_864;
pub const EVENTS: usize = 4096;
pub const DIAGNOSTIC_BYTES: usize = 262_144;
pub const COMMAND_MS: u64 = 30_000;
pub const ATTEMPT_MS: u64 = 120_000;

// Mechanism 17: bounded collector workload, independent of qualification corpus.
pub const WORKLOAD_INPUTS: usize = 16;
pub const WORKLOAD_INPUT_BYTES: usize = 16_777_216;
pub const WORKLOAD_OUTPUT_BYTES: usize = 33_554_432;
