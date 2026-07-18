use crate::dom_patch::PatchKey;
use crate::html5::tree_builder::modes::InsertionMode;

/// The only insertion modes representable on the HTML template mode stack.
#[allow(
    clippy::enum_variant_names,
    reason = "variants intentionally retain the pinned HTML insertion-mode names"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TemplateInsertionMode {
    InTemplate,
    InBody,
    InTable,
    InColumnGroup,
    InTableBody,
    InRow,
}

impl TemplateInsertionMode {
    pub(crate) fn as_insertion_mode(self) -> InsertionMode {
        match self {
            Self::InTemplate => InsertionMode::InTemplate,
            Self::InBody => InsertionMode::InBody,
            Self::InTable => InsertionMode::InTable,
            Self::InColumnGroup => InsertionMode::InColumnGroup,
            Self::InTableBody => InsertionMode::InTableBody,
            Self::InRow => InsertionMode::InRow,
        }
    }

    pub(crate) fn digest_tag(self) -> u8 {
        self.as_insertion_mode().digest_tag()
    }
}

impl TryFrom<InsertionMode> for TemplateInsertionMode {
    type Error = ();

    fn try_from(mode: InsertionMode) -> Result<Self, Self::Error> {
        match mode {
            InsertionMode::InTemplate => Ok(Self::InTemplate),
            InsertionMode::InBody => Ok(Self::InBody),
            InsertionMode::InTable => Ok(Self::InTable),
            InsertionMode::InColumnGroup => Ok(Self::InColumnGroup),
            InsertionMode::InTableBody => Ok(Self::InTableBody),
            InsertionMode::InRow => Ok(Self::InRow),
            _ => Err(()),
        }
    }
}

/// One owner-aware entry in the HTML template insertion-mode stack.
///
/// The owner is parser identity, not a DOM/public-API reference. Replacing the
/// current mode preserves the owner of the innermost open template.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::html5::tree_builder) struct TemplateModeEntry {
    owner: PatchKey,
    mode: TemplateInsertionMode,
}

impl TemplateModeEntry {
    pub(in crate::html5::tree_builder) fn new(owner: PatchKey) -> Self {
        Self {
            owner,
            mode: TemplateInsertionMode::InTemplate,
        }
    }

    pub(in crate::html5::tree_builder) fn owner(self) -> PatchKey {
        self.owner
    }

    pub(in crate::html5::tree_builder) fn mode(self) -> TemplateInsertionMode {
        self.mode
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::html5::tree_builder) struct TemplateModeStack {
    entries: Vec<TemplateModeEntry>,
}

impl TemplateModeStack {
    pub(in crate::html5::tree_builder) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(in crate::html5::tree_builder) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(in crate::html5::tree_builder) fn try_reserve_one(&mut self) -> Result<(), ()> {
        self.entries.try_reserve(1).map_err(|_| ())
    }

    pub(in crate::html5::tree_builder) fn push(&mut self, owner: PatchKey) {
        self.entries.push(TemplateModeEntry::new(owner));
    }

    pub(in crate::html5::tree_builder) fn current(&self) -> Option<TemplateModeEntry> {
        self.entries.last().copied()
    }

    pub(in crate::html5::tree_builder) fn replace_current(
        &mut self,
        mode: TemplateInsertionMode,
    ) -> Option<TemplateModeEntry> {
        let current = self.entries.last_mut()?;
        current.mode = mode;
        Some(*current)
    }

    pub(in crate::html5::tree_builder) fn pop(&mut self) -> Option<TemplateModeEntry> {
        self.entries.pop()
    }

    pub(in crate::html5::tree_builder) fn entries(&self) -> &[TemplateModeEntry] {
        &self.entries
    }

    #[cfg(test)]
    pub(in crate::html5::tree_builder) fn corrupt_current_owner_for_test(
        &mut self,
        owner: PatchKey,
    ) {
        self.entries.last_mut().expect("open template mode").owner = owner;
    }

    #[cfg(any(test, feature = "internal-api", feature = "html5-fuzzing"))]
    pub(in crate::html5::tree_builder) fn snapshot(
        &self,
    ) -> Vec<(PatchKey, TemplateInsertionMode)> {
        self.entries
            .iter()
            .map(|entry| (entry.owner, entry.mode))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::TemplateInsertionMode;
    use crate::html5::tree_builder::modes::InsertionMode;

    #[test]
    fn general_only_modes_cannot_convert_into_template_stack_modes() {
        for mode in [
            InsertionMode::Initial,
            InsertionMode::BeforeHead,
            InsertionMode::Text,
            InsertionMode::AfterBody,
        ] {
            assert!(TemplateInsertionMode::try_from(mode).is_err());
        }
    }
}
