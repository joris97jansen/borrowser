mod apply;
mod arena;
mod invariants;
mod materialize;

#[cfg(test)]
mod tests;

type ArenaResult<T> = Result<T, String>;

pub(crate) use arena::TestPatchArena;
