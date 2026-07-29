#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
use std::num::NonZeroU64;

use super::{ParserReservationSite, ParserResourceExhaustion};

/// Deterministic parser-local failure selected for one semantic reservation
/// boundary.
#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParserFailureInjection {
    site: ParserReservationSite,
    occurrence: NonZeroU64,
}

#[cfg(all(
    feature = "parser-failure-injection",
    any(test, feature = "internal-api")
))]
impl ParserFailureInjection {
    pub const fn new(site: ParserReservationSite, occurrence: NonZeroU64) -> Self {
        Self { site, occurrence }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ParserReservationController {
    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    selected: Option<ParserFailureInjection>,
    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    matching_occurrences: u64,
}

impl ParserReservationController {
    #[cfg(all(
        feature = "parser-failure-injection",
        any(test, feature = "internal-api")
    ))]
    pub(crate) const fn with_failure(injection: ParserFailureInjection) -> Self {
        Self {
            selected: Some(injection),
            matching_occurrences: 0,
        }
    }

    #[inline]
    pub(crate) fn before_reservation(
        &mut self,
        site: ParserReservationSite,
    ) -> Result<(), ParserResourceExhaustion> {
        #[cfg(all(
            feature = "parser-failure-injection",
            any(test, feature = "internal-api")
        ))]
        if let Some(selected) = self.selected
            && selected.site == site
        {
            self.matching_occurrences += 1;
            if self.matching_occurrences == selected.occurrence.get() {
                self.selected = None;
                return Err(ParserResourceExhaustion::at(site));
            }
        }

        let _ = site;
        Ok(())
    }
}

#[cfg(all(test, feature = "parser-failure-injection"))]
mod tests {
    use super::*;

    #[test]
    fn selected_occurrence_fires_once_and_controllers_remain_parser_local() {
        let selected = ParserFailureInjection::new(
            ParserReservationSite::TemplateChildStorage,
            NonZeroU64::new(2).expect("non-zero"),
        );
        let mut first = ParserReservationController::with_failure(selected);
        let mut second = ParserReservationController::with_failure(selected);

        assert_eq!(
            first.before_reservation(ParserReservationSite::KnownTagAtomStorage),
            Ok(())
        );
        assert_eq!(
            first.before_reservation(ParserReservationSite::TemplateChildStorage),
            Ok(())
        );
        assert_eq!(
            second.before_reservation(ParserReservationSite::TemplateChildStorage),
            Ok(())
        );
        assert!(matches!(
            first.before_reservation(ParserReservationSite::TemplateChildStorage),
            Err(error) if error.site() == ParserReservationSite::TemplateChildStorage
        ));
        assert_eq!(
            first.before_reservation(ParserReservationSite::TemplateChildStorage),
            Ok(()),
            "a fired injection must disarm itself"
        );
        assert!(matches!(
            second.before_reservation(ParserReservationSite::TemplateChildStorage),
            Err(error) if error.site() == ParserReservationSite::TemplateChildStorage
        ));
        assert_eq!(
            second.before_reservation(ParserReservationSite::TemplateChildStorage),
            Ok(())
        );
    }
}

#[cfg(all(test, not(feature = "parser-failure-injection")))]
mod disabled_tests {
    use super::ParserReservationController;

    #[test]
    fn disabled_controller_has_no_failure_state_or_occurrence_counter() {
        assert_eq!(std::mem::size_of::<ParserReservationController>(), 0);
    }
}
