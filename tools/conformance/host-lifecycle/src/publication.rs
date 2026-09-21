//! Publication priority is derived from the event, never selected by its caller.
use crate::{model::Event, scheduling::EndpointClass};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationClass {
    Ordinary,
    Recovery,
}
impl Event {
    pub(crate) fn publication_class(&self) -> PublicationClass {
        use Event::*;
        use PublicationClass::*;
        match self {
            // Retain mutation outcomes, ownership and reviewed cleanup facts.
            BaselineAttributionResolved { .. }
            | BaselineCandidateDisqualified { .. }
            | CancellationRetryResolved { .. }
            | HistoryConflictObserved { .. }
            | AllocationResponse { .. }
            | PartialTransactionIdentity { .. }
            | TransactionObserved { .. }
            | ServerObserved { .. }
            | CancellationAuthorized { .. }
            | CancellationDispatchIntent { .. }
            | CancellationObserved { .. }
            | CancellationResubmissionAuthorized { .. }
            | AllocationResolved { .. }
            | IdentityConflictResolved { .. }
            | NonAllocationResolved { .. }
            | ReleaseResolved { .. }
            | AuthenticationResolved { .. }
            | ProviderAccessResolved { .. }
            | MutationResponseLost { .. } => Recovery,
            // Cancellation's prerequisite reads and accounting must remain possible.
            EndpointCharged { endpoint }
            | ReadSucceeded { endpoint }
            | BudgetRebootHold { endpoint } => match endpoint {
                EndpointClass::Cancellation | EndpointClass::CancellationRead => Recovery,
                EndpointClass::Allocation
                | EndpointClass::TransactionHistory
                | EndpointClass::Transaction
                | EndpointClass::Server
                | EndpointClass::Catalogue => Ordinary,
            },
            FailureObserved { endpoint, .. } => match endpoint {
                EndpointClass::Allocation
                | EndpointClass::Cancellation
                | EndpointClass::CancellationRead => Recovery,
                EndpointClass::TransactionHistory
                | EndpointClass::Transaction
                | EndpointClass::Server
                | EndpointClass::Catalogue => Ordinary,
            },
            AuthorityInitialized
            | OperationAuthorized { .. }
            | CatalogueObserved { .. }
            | BaselineStarted
            | BaselineTransaction { .. }
            | BaselineServer { .. }
            | BaselineCompleted { .. }
            | AllocationDispatchIntent { .. }
            | ObservationRoundStarted
            | WatchProgress { .. } => Ordinary,
        }
    }
}
