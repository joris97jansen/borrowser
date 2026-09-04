#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReservationSite {
    Phase4Captures,
    Phase4Tracks,
    Phase4Attachments,
    Phase4Notes,
    Phase5Captures,
    Phase6Captures,
    Phase8Attachments,
    Phase8Notes,
}

pub(super) trait ReservationPolicy {
    fn before_reserve(&mut self, site: ReservationSite) -> Result<(), ()>;
}

pub(super) struct ProductionReservation;

impl ReservationPolicy for ProductionReservation {
    fn before_reserve(&mut self, _site: ReservationSite) -> Result<(), ()> {
        Ok(())
    }
}

pub(super) fn try_reserve_vec<T>(
    values: &mut Vec<T>,
    additional: usize,
    site: ReservationSite,
    policy: &mut impl ReservationPolicy,
) -> Result<(), ()> {
    policy.before_reserve(site)?;
    values.try_reserve(additional).map_err(|_| ())
}

#[cfg(test)]
pub(super) struct RejectReservationAt {
    site: ReservationSite,
}

#[cfg(test)]
impl RejectReservationAt {
    pub(super) const fn new(site: ReservationSite) -> Self {
        Self { site }
    }
}

#[cfg(test)]
impl ReservationPolicy for RejectReservationAt {
    fn before_reserve(&mut self, site: ReservationSite) -> Result<(), ()> {
        if site == self.site { Err(()) } else { Ok(()) }
    }
}
