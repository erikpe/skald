//! Unit-bearing report metrics.

/// The integer value and unit of a report metric.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricValue {
    Count(u64),
    Bytes(u64),
}

/// One owner-named metric.
///
/// A phase or pass emits metrics in its documented deterministic order. The
/// reporting layer preserves that order and does not sort by presentation
/// label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportMetric {
    owner: Option<&'static str>,
    name: &'static str,
    value: MetricValue,
}

impl ReportMetric {
    pub const fn new(name: &'static str, value: MetricValue) -> Self {
        Self {
            owner: None,
            name,
            value,
        }
    }

    pub const fn count(name: &'static str, value: u64) -> Self {
        Self::new(name, MetricValue::Count(value))
    }

    pub const fn bytes(name: &'static str, value: u64) -> Self {
        Self::new(name, MetricValue::Bytes(value))
    }

    pub(crate) const fn pass_count(
        pass_name: &'static str,
        name: &'static str,
        value: u64,
    ) -> Self {
        Self {
            owner: Some(pass_name),
            name,
            value: MetricValue::Count(value),
        }
    }

    pub const fn owner(&self) -> Option<&'static str> {
        self.owner
    }

    pub const fn name(&self) -> &'static str {
        self.name
    }

    pub const fn value(&self) -> MetricValue {
        self.value
    }
}
