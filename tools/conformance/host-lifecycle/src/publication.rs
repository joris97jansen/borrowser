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
            Self::AuthorityInitialized
            | Self::LaunchPrepared(_)
            | Self::LaunchDispatchIntent(_)
            | Self::LaunchAttemptIntent(_) => PublicationClass::Ordinary,
            // Records an already consumed attempt, never grants another one.
            // At most three such outcomes per unresolved operation.
            Self::LaunchAttemptOutcome(_) => PublicationClass::Recovery,
            #[cfg(test)]
            Self::StorageCheckpoint => PublicationClass::Ordinary,
            #[cfg(test)]
            Self::StorageEvidence { .. } => PublicationClass::Ordinary,
            #[cfg(test)]
            Self::StorageRecovery => PublicationClass::Recovery,
        }
    }
}
