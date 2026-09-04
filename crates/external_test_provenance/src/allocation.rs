#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReservationSite {
    ConfinedReadInitial,
    ConfinedReadGrowth,
    ComparableDomStack,
    ComparableDomString,
    CanonicalIdentity,
}

pub(crate) trait ReservationPolicy {
    fn before_reserve(&mut self, site: ReservationSite) -> Result<(), ()>;
}

pub(crate) struct ProductionReservation;

impl ReservationPolicy for ProductionReservation {
    fn before_reserve(&mut self, _site: ReservationSite) -> Result<(), ()> {
        Ok(())
    }
}

pub(crate) fn try_reserve_vec<T>(
    values: &mut Vec<T>,
    additional: usize,
    site: ReservationSite,
    policy: &mut impl ReservationPolicy,
) -> Result<(), ()> {
    policy.before_reserve(site)?;
    values.try_reserve(additional).map_err(|_| ())
}

pub(crate) fn try_reserve_string(
    value: &mut String,
    additional: usize,
    site: ReservationSite,
    policy: &mut impl ReservationPolicy,
) -> Result<(), ()> {
    policy.before_reserve(site)?;
    value.try_reserve(additional).map_err(|_| ())
}

#[cfg(test)]
pub(crate) struct RejectReservationAt {
    site: ReservationSite,
}

#[cfg(test)]
impl RejectReservationAt {
    pub(crate) const fn new(site: ReservationSite) -> Self {
        Self { site }
    }
}

#[cfg(test)]
impl ReservationPolicy for RejectReservationAt {
    fn before_reserve(&mut self, site: ReservationSite) -> Result<(), ()> {
        (site != self.site).then_some(()).ok_or(())
    }
}
