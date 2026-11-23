use super::*;

/// Coverage ignores guards. Ordered evaluation also removes matched inputs when a guard cannot return.
pub(in super::super) struct MatchDomain {
    pub(in super::super) constructors: Option<[CanonicalTarget; 2]>,
    pub(in super::super) impossible: [bool; 2],
    pub(in super::super) carrier: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Portion {
    None,
    Part,
    All,
}

pub(super) struct ArmSearch {
    pub(super) domain: MatchDomain,
    remaining: [Portion; 2],
    matched: [Portion; 2],
    uncertain_entry: bool,
}

impl ArmSearch {
    pub(super) fn new(domain: MatchDomain) -> Self {
        Self {
            remaining: domain
                .impossible
                .map(|absent| if absent { Portion::None } else { Portion::All }),
            domain,
            matched: [Portion::None; 2],
            uncertain_entry: false,
        }
    }

    pub(super) fn enter(&mut self, pattern: &PatternCheck) -> NormalContinuation {
        self.matched = std::array::from_fn(|i| {
            if self.remaining[i] == Portion::None {
                return Portion::None;
            }
            if pattern.coverage.total {
                return Portion::All;
            }
            let Some(roles) = &self.domain.constructors else {
                return Portion::Part;
            };
            if pattern.coverage.variants.contains(&roles[i]) {
                Portion::All
            } else if pattern.partial_variant.as_ref() == Some(&roles[i])
                || (pattern.partial_variant.is_none() && pattern.coverage.variants.is_empty())
            {
                Portion::Part
            } else {
                Portion::None
            }
        });
        self.uncertain_entry = self
            .matched
            .iter()
            .zip(self.remaining)
            .all(|(part, remaining)| *part == Portion::None || remaining == Portion::Part)
            && self.matched != [Portion::None; 2];
        if self.matched == [Portion::None; 2] {
            NormalContinuation::Absent
        } else {
            NormalContinuation::Present
        }
    }

    /// Only misses continue. A partial head retains an unknown subset of its constructor.
    pub(super) fn keep_misses(&mut self) {
        for (remaining, portion) in self.remaining.iter_mut().zip(self.matched) {
            if portion == Portion::All {
                *remaining = Portion::None;
            }
        }
    }

    pub(super) fn stop_matched(&mut self) {
        self.keep_misses();
        for (remaining, portion) in self.remaining.iter_mut().zip(self.matched) {
            if portion == Portion::Part {
                *remaining = Portion::Part;
            }
        }
    }

    pub(super) fn uncertain_entry(&self) -> bool {
        self.uncertain_entry
    }
}
