use crate::{
    platform::{Duration, Instant},
    TimeSpan,
};
use core::ops::Sub;

/// A Time Stamp stores a point in time, that can be used to calculate Time
/// Spans.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct TimeStamp(Instant);

impl TimeStamp {
    /// Creates a new Time Stamp, representing the current point in time.
    pub fn now() -> Self {
        TimeStamp(Instant::now())
    }
}

impl Sub for TimeStamp {
    type Output = TimeSpan;

    fn sub(self, rhs: TimeStamp) -> TimeSpan {
        TimeSpan::from(self.0 - rhs.0)
    }
}

impl Sub<TimeSpan> for TimeStamp {
    type Output = TimeStamp;

    fn sub(self, rhs: TimeSpan) -> TimeStamp {
        TimeStamp(self.0 - Duration::from(rhs))
    }
}
