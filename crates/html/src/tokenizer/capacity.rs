pub(crate) const TOKEN_CAPACITY_MIN: usize = 16;
pub(crate) const TEXT_POOL_CAPACITY_MIN: usize = 4;

#[inline]
pub(crate) fn estimate_token_capacity(input_len: usize) -> usize {
    (input_len / 8).saturating_add(TOKEN_CAPACITY_MIN)
}

#[inline]
pub(crate) fn estimate_text_pool_capacity(input_len: usize) -> usize {
    (input_len / 64).saturating_add(TEXT_POOL_CAPACITY_MIN)
}
