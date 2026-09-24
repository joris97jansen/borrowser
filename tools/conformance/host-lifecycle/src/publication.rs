//! Reserve classification belongs to typed events, never a caller flag.
use crate::model::EventV2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationClass {
    Ordinary,
    Recovery,
}
impl EventV2 {
    pub(crate) fn publication_class(&self) -> PublicationClass {
        match self {
            Self::AuthorityInitialized => PublicationClass::Ordinary,
            #[cfg(test)]
            Self::StorageCheckpoint => PublicationClass::Ordinary,
            #[cfg(test)]
            Self::StorageEvidence { .. } => PublicationClass::Ordinary,
            #[cfg(test)]
            Self::StorageRecovery => PublicationClass::Recovery,
        }
    }
}
