use super::api::Html5ParseSession;

impl Html5ParseSession {
    pub(super) fn sync_debug_counters(&mut self) {
        self.ctx.counters.max_open_elements_depth = self
            .ctx
            .counters
            .max_open_elements_depth
            .max(self.builder.max_open_elements_depth());
        self.ctx.counters.max_active_formatting_depth = self
            .ctx
            .counters
            .max_active_formatting_depth
            .max(self.builder.max_active_formatting_depth());
        // Builder perf counters are cumulative session-lifetime totals.
        // Copying by assignment keeps counters authoritative to builder state
        // (this is intentionally not delta accumulation per pump call).
        self.ctx.counters.soe_push_ops = self.builder.perf_soe_push_ops();
        self.ctx.counters.soe_pop_ops = self.builder.perf_soe_pop_ops();
        self.ctx.counters.soe_scope_scan_calls = self.builder.perf_soe_scope_scan_calls();
        self.ctx.counters.soe_scope_scan_steps = self.builder.perf_soe_scope_scan_steps();
        self.ctx.counters.tree_builder_patches_emitted = self.builder.perf_patches_emitted();
        self.ctx.counters.tree_builder_text_nodes_created = self.builder.perf_text_nodes_created();
        self.ctx.counters.tree_builder_text_appends = self.builder.perf_text_appends();
        self.ctx.counters.tree_builder_text_coalescing_invalidations =
            self.builder.perf_text_coalescing_invalidations();
    }
}
