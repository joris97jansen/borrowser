use html::chunker::{ChunkPlanCase, ChunkerConfig, build_chunk_plans};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FixtureBand {
    pub(crate) names: &'static [&'static str],
    fuzz_runs: usize,
    fuzz_seed: u64,
}

impl FixtureBand {
    pub(crate) fn chunk_plans(self, input: &str) -> Vec<ChunkPlanCase> {
        build_chunk_plans(input, self.fuzz_runs, self.fuzz_seed, ChunkerConfig::utf8())
    }
}

pub(crate) const H8_FIXTURE_NAMES: &[&str] = &[
    "h8-nested-supported-inline",
    "h8-nested-supported-inline-attrs",
    "h8-aaa-misnest-b-i",
    "h8-aaa-misnest-a-b-attrs",
    "h8-reconstruct-single-comment-tail",
    "h8-reconstruct-multi-comment-tail",
    "h8-special-anchor-comment-tail",
    "h8-special-nobr-comment-tail",
    "h8-marker-applet-formatting-isolation",
];

pub(crate) const H10_FIXTURE_NAMES: &[&str] = &[
    "h10-aaa-furthest-block-reparent",
    "h10-aaa-foster-parent-insert-before",
];

pub(crate) const I10_TABLE_FIXTURE_NAMES: &[&str] = &[
    "i10-table-normal-sections",
    "i10-table-missing-tbody-implied",
    "i10-table-missing-cell-end-tags",
    "i10-table-stray-text-foster-parent",
    "i10-table-stray-tag-foster-parent",
    "i10-table-nested-basic",
];

pub(crate) const H8_FIXTURE_BAND: FixtureBand = FixtureBand {
    names: H8_FIXTURE_NAMES,
    fuzz_runs: 1,
    fuzz_seed: 0xC0FFEE,
};

pub(crate) const H10_FIXTURE_BAND: FixtureBand = FixtureBand {
    names: H10_FIXTURE_NAMES,
    fuzz_runs: 1,
    fuzz_seed: 0xC0FFEE,
};

pub(crate) const I10_TABLE_FIXTURE_BAND: FixtureBand = FixtureBand {
    names: I10_TABLE_FIXTURE_NAMES,
    fuzz_runs: 4,
    fuzz_seed: 0x10C0DE,
};
