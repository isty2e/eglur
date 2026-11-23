use super::*;

pub(in super::super) enum Coverage {
    Exhaustive,
    Unreachable(PatternId),
    MissingVariant,
    Unknown,
}

pub(super) struct CheckedMatchPatterns {
    pub(super) carriers: Option<Vec<CarrierArm>>,
    pub(super) coverage: Coverage,
}

impl MatchDomain {
    pub(super) fn check_patterns(
        &self,
        arms: &[eglur_hir::MatchArm],
        patterns: &[PatternCheck],
    ) -> CheckedMatchPatterns {
        assert_eq!(arms.len(), patterns.len(), "all arms passed pattern typing");
        let impossible = self.impossible;
        let universe = self.constructors.as_ref().map(|roles| roles.each_ref());
        let mut carrier_arms =
            (self.carrier && universe.is_some()).then(|| Vec::with_capacity(arms.len()));
        let mut total = universe.is_some() && impossible == [true, true];
        let mut variants = BTreeSet::new();
        if let Some(roles) = universe {
            for (role, impossible) in roles.into_iter().zip(impossible) {
                if impossible {
                    variants.insert(role.clone());
                }
            }
        }

        let mut first_unreachable = None;
        for (arm, pattern) in arms.iter().zip(patterns) {
            let coverage = &pattern.coverage;
            let unreachable = total
                || pattern
                    .partial_variant
                    .as_ref()
                    .is_some_and(|variant| variants.contains(variant))
                || (!coverage.variants.is_empty()
                    && coverage
                        .variants
                        .iter()
                        .all(|variant| variants.contains(variant)));
            if unreachable {
                first_unreachable.get_or_insert(arm.pattern);
            }
            if let (Some(roles), Some(facts)) = (universe, &mut carrier_arms) {
                if let Some(fact) = pattern.carrier_arm(roles, &variants) {
                    facts.push(fact);
                } else {
                    carrier_arms = None;
                }
            }
            if arm.guard.is_none() {
                variants.extend(coverage.variants.iter().cloned());
                total |= coverage.total
                    || universe
                        .is_some_and(|roles| roles.into_iter().all(|v| variants.contains(v)));
            }
        }
        let coverage = if let Some(pattern) = first_unreachable {
            Coverage::Unreachable(pattern)
        } else if total {
            Coverage::Exhaustive
        } else if universe.is_some() {
            Coverage::MissingVariant
        } else {
            Coverage::Unknown
        };
        CheckedMatchPatterns {
            carriers: carrier_arms,
            coverage,
        }
    }
}

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
