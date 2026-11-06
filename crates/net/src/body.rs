use core_types::ResourceKind;

use crate::{NetworkErrorKind, limits::resource_byte_limit};

/// Delivered content bytes, not encoded length or proof of a complete representation.
/// The route determines the cap; callers cannot supply an independent limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyProgress {
    route: ResourceKind,
    delivered_bytes: usize,
}

impl BodyProgress {
    pub fn try_new(route: ResourceKind, delivered_bytes: usize) -> Result<Self, NetworkErrorKind> {
        if delivered_bytes > resource_byte_limit(route) {
            return Err(NetworkErrorKind::ResourceLimit);
        }
        Ok(Self {
            route,
            delivered_bytes,
        })
    }

    pub fn route(self) -> ResourceKind {
        self.route
    }
    pub fn delivered_bytes(self) -> usize {
        self.delivered_bytes
    }
    pub fn limit_bytes(self) -> usize {
        resource_byte_limit(self.route)
    }
    pub fn matches_route(self, route: ResourceKind) -> bool {
        self.route == route
    }
}
