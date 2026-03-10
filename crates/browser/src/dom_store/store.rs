use super::document::DomDoc;
use super::error::DomPatchError;
use core_types::{DomHandle, DomVersion};
use html::{DomPatch, Node};
use std::collections::HashMap;

pub struct DomStore {
    docs: HashMap<DomHandle, DomDoc>,
}

impl DomStore {
    pub fn new() -> Self {
        Self {
            docs: HashMap::new(),
        }
    }

    pub fn create(&mut self, handle: DomHandle) -> Result<(), DomPatchError> {
        if self.docs.contains_key(&handle) {
            return Err(DomPatchError::DuplicateHandle(handle));
        }
        self.docs.insert(handle, DomDoc::new());
        Ok(())
    }

    pub fn drop_handle(&mut self, handle: DomHandle) {
        self.docs.remove(&handle);
    }

    pub fn clear(&mut self) {
        self.docs.clear();
    }

    pub fn apply(
        &mut self,
        handle: DomHandle,
        from: DomVersion,
        to: DomVersion,
        patches: &[DomPatch],
    ) -> Result<(), DomPatchError> {
        if patches.is_empty() {
            return Err(DomPatchError::Protocol("empty patch batch"));
        }
        let doc = self
            .docs
            .get_mut(&handle)
            .ok_or(DomPatchError::UnknownHandle(handle))?;
        if doc.version != from {
            return Err(DomPatchError::VersionMismatch {
                expected: doc.version,
                got: from,
            });
        }
        if to != from.next() {
            return Err(DomPatchError::NonMonotonicVersion { from, to });
        }
        // Apply transactionally: on any protocol/runtime error, keep the
        // previous document state unchanged.
        let mut staged = doc.clone();
        staged.apply(patches)?;
        staged.version = to;
        staged.rebuild_cache()?;
        *doc = staged;
        Ok(())
    }

    pub fn get_current(&self, handle: DomHandle) -> Option<&Node> {
        self.docs
            .get(&handle)
            .and_then(|doc| doc.current.as_deref())
    }

    pub fn materialize(&self, handle: DomHandle) -> Result<Box<Node>, DomPatchError> {
        Ok(Box::new(self.materialize_owned(handle)?))
    }

    pub fn materialize_owned(&self, handle: DomHandle) -> Result<Node, DomPatchError> {
        let doc = self
            .docs
            .get(&handle)
            .ok_or(DomPatchError::UnknownHandle(handle))?;
        doc.materialize_owned()
    }
}

impl Default for DomStore {
    fn default() -> Self {
        Self::new()
    }
}
