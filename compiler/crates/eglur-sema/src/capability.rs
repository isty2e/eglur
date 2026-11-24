use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{ExternalDefId, SemanticLocation};

#[cfg(test)]
mod tests;

/// Independently selectable operations owned by name resolution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResolveCapability {
    CollectDeclarations,
    ResolveFileSetImports,
    ResolveLexicalValues,
    ResolveModuleValues,
    ResolveQualifiedValues,
    ResolvePatternScopes,
    ResolveAttributePaths,
    ResolveTrustedExternalNames,
    ResolveCompleteStandardLibraryNames,
}

impl ResolveCapability {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::CollectDeclarations => "resolve.collect_declarations",
            Self::ResolveFileSetImports => "resolve.file_set_imports",
            Self::ResolveLexicalValues => "resolve.lexical_values",
            Self::ResolveModuleValues => "resolve.module_values",
            Self::ResolveQualifiedValues => "resolve.qualified_values",
            Self::ResolvePatternScopes => "resolve.pattern_scopes",
            Self::ResolveAttributePaths => "resolve.attribute_paths",
            Self::ResolveTrustedExternalNames => "resolve.trusted_external_names",
            Self::ResolveCompleteStandardLibraryNames => "resolve.complete_stdlib_names",
        }
    }
}

/// Independently selectable operations owned by type-oriented semantic phases.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypecheckCapability {
    ResolveDeclarationSignatures,
    ResolveTraitEnvironment,
    CheckBodies,
    ResolveTraitEvidence,
    ComputeEffectSummaries,
    ResolveMembers,
    ResolveMethods,
    ResolveTrustedInherentMethods,
    ResolveOverloads,
    CheckOperators,
    CheckSelectedCheckedIntegerArithmetic,
    EvaluateSelectedIntegerConstExpressions,
    EvaluateSelectedIntegerConstDeclarations,
    EvaluateScalarConstExpressions,
    CheckBranching,
    AnalyzePatternCoverage,
    CheckAdvancedPatterns,
    CheckLoops,
    CheckFailureFlow,
    InferResultBlockType,
    CheckClosures,
    CheckContainers,
    CheckOwnershipFlow,
    CheckCompleteOwnershipFlow,
    CheckUnsafeExpressions,
    AnalyzeInhabitance,
    AnalyzeTensorSource,
    CheckTensorConstructors,
}

impl TypecheckCapability {
    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::ResolveDeclarationSignatures => "typecheck.declaration_signatures",
            Self::ResolveTraitEnvironment => "typecheck.trait_environment",
            Self::CheckBodies => "typecheck.bodies",
            Self::ResolveTraitEvidence => "typecheck.trait_evidence",
            Self::ComputeEffectSummaries => "typecheck.effect_summaries",
            Self::ResolveMembers => "typecheck.members",
            Self::ResolveMethods => "typecheck.methods",
            Self::ResolveTrustedInherentMethods => "typecheck.trusted_inherent_methods",
            Self::ResolveOverloads => "typecheck.overloads",
            Self::CheckOperators => "typecheck.operators",
            Self::CheckSelectedCheckedIntegerArithmetic => {
                "typecheck.selected_checked_integer_arithmetic"
            }
            Self::EvaluateScalarConstExpressions => "typecheck.scalar_const_evaluation",
            Self::EvaluateSelectedIntegerConstExpressions => "typecheck.integer_const_expressions",
            Self::EvaluateSelectedIntegerConstDeclarations => {
                "typecheck.integer_const_declarations"
            }
            Self::CheckBranching => "typecheck.branching",
            Self::AnalyzePatternCoverage => "typecheck.pattern_coverage",
            Self::CheckAdvancedPatterns => "typecheck.advanced_patterns",
            Self::CheckLoops => "typecheck.loops",
            Self::CheckFailureFlow => "typecheck.failure_flow",
            Self::InferResultBlockType => "typecheck.result_block_inference",
            Self::CheckClosures => "typecheck.closures",
            Self::CheckContainers => "typecheck.containers",
            Self::CheckOwnershipFlow => "typecheck.ownership_flow",
            Self::CheckCompleteOwnershipFlow => "typecheck.complete_ownership_flow",
            Self::CheckUnsafeExpressions => "typecheck.unsafe_expressions",
            Self::AnalyzeInhabitance => "typecheck.inhabitance",
            Self::AnalyzeTensorSource => "typecheck.tensor_source",
            Self::CheckTensorConstructors => "typecheck.tensor_constructors",
        }
    }
}

/// Closed owner-partitioned semantic compile capability.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompileCapability {
    Resolve(ResolveCapability),
    Typecheck(TypecheckCapability),
}

impl CompileCapability {
    /// Complete closed capability vocabulary for this compiler revision.
    pub const ALL: [Self; 37] = [
        Self::Resolve(ResolveCapability::CollectDeclarations),
        Self::Resolve(ResolveCapability::ResolveFileSetImports),
        Self::Resolve(ResolveCapability::ResolveLexicalValues),
        Self::Resolve(ResolveCapability::ResolveModuleValues),
        Self::Resolve(ResolveCapability::ResolveQualifiedValues),
        Self::Resolve(ResolveCapability::ResolvePatternScopes),
        Self::Resolve(ResolveCapability::ResolveAttributePaths),
        Self::Resolve(ResolveCapability::ResolveTrustedExternalNames),
        Self::Resolve(ResolveCapability::ResolveCompleteStandardLibraryNames),
        Self::Typecheck(TypecheckCapability::ResolveDeclarationSignatures),
        Self::Typecheck(TypecheckCapability::ResolveTraitEnvironment),
        Self::Typecheck(TypecheckCapability::CheckBodies),
        Self::Typecheck(TypecheckCapability::ResolveTraitEvidence),
        Self::Typecheck(TypecheckCapability::ComputeEffectSummaries),
        Self::Typecheck(TypecheckCapability::ResolveMembers),
        Self::Typecheck(TypecheckCapability::ResolveMethods),
        Self::Typecheck(TypecheckCapability::ResolveTrustedInherentMethods),
        Self::Typecheck(TypecheckCapability::ResolveOverloads),
        Self::Typecheck(TypecheckCapability::CheckOperators),
        Self::Typecheck(TypecheckCapability::CheckSelectedCheckedIntegerArithmetic),
        Self::Typecheck(TypecheckCapability::EvaluateScalarConstExpressions),
        Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstExpressions),
        Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstDeclarations),
        Self::Typecheck(TypecheckCapability::CheckBranching),
        Self::Typecheck(TypecheckCapability::AnalyzePatternCoverage),
        Self::Typecheck(TypecheckCapability::CheckAdvancedPatterns),
        Self::Typecheck(TypecheckCapability::CheckLoops),
        Self::Typecheck(TypecheckCapability::CheckFailureFlow),
        Self::Typecheck(TypecheckCapability::InferResultBlockType),
        Self::Typecheck(TypecheckCapability::CheckClosures),
        Self::Typecheck(TypecheckCapability::CheckContainers),
        Self::Typecheck(TypecheckCapability::CheckOwnershipFlow),
        Self::Typecheck(TypecheckCapability::CheckCompleteOwnershipFlow),
        Self::Typecheck(TypecheckCapability::CheckUnsafeExpressions),
        Self::Typecheck(TypecheckCapability::AnalyzeInhabitance),
        Self::Typecheck(TypecheckCapability::AnalyzeTensorSource),
        Self::Typecheck(TypecheckCapability::CheckTensorConstructors),
    ];

    #[must_use]
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::Resolve(capability) => capability.stable_key(),
            Self::Typecheck(capability) => capability.stable_key(),
        }
    }

    /// Returns intrinsic capability prerequisites in deterministic order.
    #[must_use]
    pub const fn prerequisites(self) -> &'static [Self] {
        const COLLECT: &[CompileCapability] = &[CompileCapability::Resolve(
            ResolveCapability::CollectDeclarations,
        )];
        const EXTERNAL_NAMES: &[CompileCapability] = &[CompileCapability::Resolve(
            ResolveCapability::ResolveTrustedExternalNames,
        )];
        const DECLARATION_SIGNATURES: &[CompileCapability] = &[CompileCapability::Typecheck(
            TypecheckCapability::ResolveDeclarationSignatures,
        )];
        const TRAIT_ENVIRONMENT: &[CompileCapability] = &[CompileCapability::Typecheck(
            TypecheckCapability::ResolveTraitEnvironment,
        )];
        const BODIES: &[CompileCapability] = &[CompileCapability::Typecheck(
            TypecheckCapability::CheckBodies,
        )];
        const TRAIT_EVIDENCE: &[CompileCapability] = &[
            CompileCapability::Typecheck(TypecheckCapability::CheckBodies),
            CompileCapability::Typecheck(TypecheckCapability::ResolveTraitEvidence),
        ];
        const FAILURE_FLOW: &[CompileCapability] = &[
            CompileCapability::Typecheck(TypecheckCapability::CheckBodies),
            CompileCapability::Typecheck(TypecheckCapability::CheckFailureFlow),
        ];
        match self {
            Self::Resolve(ResolveCapability::CollectDeclarations)
            | Self::Resolve(ResolveCapability::ResolveTrustedExternalNames) => &[],
            Self::Resolve(ResolveCapability::ResolveCompleteStandardLibraryNames) => EXTERNAL_NAMES,
            Self::Resolve(_)
            | Self::Typecheck(TypecheckCapability::ResolveDeclarationSignatures) => COLLECT,
            Self::Typecheck(TypecheckCapability::ResolveTraitEnvironment) => DECLARATION_SIGNATURES,
            Self::Typecheck(TypecheckCapability::CheckBodies) => TRAIT_ENVIRONMENT,
            Self::Typecheck(TypecheckCapability::ResolveTraitEvidence) => BODIES,
            Self::Typecheck(TypecheckCapability::ResolveTrustedInherentMethods) => {
                &[Self::Typecheck(TypecheckCapability::ResolveMethods)]
            }
            Self::Typecheck(TypecheckCapability::CheckCompleteOwnershipFlow) => {
                &[Self::Typecheck(TypecheckCapability::CheckOwnershipFlow)]
            }
            Self::Typecheck(TypecheckCapability::ComputeEffectSummaries) => TRAIT_EVIDENCE,
            Self::Typecheck(TypecheckCapability::CheckSelectedCheckedIntegerArithmetic) => {
                FAILURE_FLOW
            }
            Self::Typecheck(TypecheckCapability::InferResultBlockType) => FAILURE_FLOW,
            Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstExpressions) => {
                &[Self::Typecheck(
                    TypecheckCapability::CheckSelectedCheckedIntegerArithmetic,
                )]
            }
            Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstDeclarations) => {
                &[Self::Typecheck(
                    TypecheckCapability::EvaluateSelectedIntegerConstExpressions,
                )]
            }
            Self::Typecheck(
                TypecheckCapability::ResolveMembers
                | TypecheckCapability::ResolveMethods
                | TypecheckCapability::ResolveOverloads
                | TypecheckCapability::CheckOperators
                | TypecheckCapability::EvaluateScalarConstExpressions
                | TypecheckCapability::CheckBranching
                | TypecheckCapability::AnalyzePatternCoverage
                | TypecheckCapability::CheckAdvancedPatterns
                | TypecheckCapability::CheckLoops
                | TypecheckCapability::CheckFailureFlow
                | TypecheckCapability::CheckClosures
                | TypecheckCapability::CheckContainers
                | TypecheckCapability::CheckOwnershipFlow
                | TypecheckCapability::CheckUnsafeExpressions
                | TypecheckCapability::AnalyzeInhabitance
                | TypecheckCapability::AnalyzeTensorSource
                | TypecheckCapability::CheckTensorConstructors,
            ) => BODIES,
        }
    }

    /// Reports whether this compiler slice has an implementation for the operation.
    #[must_use]
    pub const fn is_implemented(self) -> bool {
        matches!(
            self,
            Self::Typecheck(TypecheckCapability::ResolveDeclarationSignatures)
                | Self::Typecheck(TypecheckCapability::ResolveTraitEnvironment)
                | Self::Typecheck(TypecheckCapability::CheckBodies)
                | Self::Typecheck(TypecheckCapability::ResolveTraitEvidence)
                | Self::Typecheck(TypecheckCapability::ComputeEffectSummaries)
                | Self::Typecheck(TypecheckCapability::ResolveMembers)
                | Self::Typecheck(TypecheckCapability::ResolveMethods)
                | Self::Typecheck(TypecheckCapability::ResolveTrustedInherentMethods)
                | Self::Typecheck(TypecheckCapability::ResolveOverloads)
                | Self::Typecheck(TypecheckCapability::CheckSelectedCheckedIntegerArithmetic)
                | Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstExpressions)
                | Self::Typecheck(TypecheckCapability::EvaluateSelectedIntegerConstDeclarations)
                | Self::Typecheck(TypecheckCapability::CheckBranching)
                | Self::Typecheck(TypecheckCapability::CheckFailureFlow)
                | Self::Typecheck(TypecheckCapability::CheckOwnershipFlow)
                | Self::Typecheck(TypecheckCapability::AnalyzeInhabitance)
                | Self::Typecheck(TypecheckCapability::CheckTensorConstructors)
                | Self::Resolve(
                    ResolveCapability::CollectDeclarations
                        | ResolveCapability::ResolveFileSetImports
                        | ResolveCapability::ResolveLexicalValues
                        | ResolveCapability::ResolveModuleValues
                        | ResolveCapability::ResolveQualifiedValues
                        | ResolveCapability::ResolvePatternScopes
                        | ResolveCapability::ResolveTrustedExternalNames
                )
        )
    }
}

/// Pure support query implemented by the driver-owned selected profile.
pub trait CompileSupport {
    fn supports(&self, capability: CompileCapability) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DemandDisposition {
    /// Selected by the profile, independently of required input availability.
    Satisfied,
    Missing,
}

/// Required catalog input, independently of the selected implementation profile.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum UnavailableCompileInput {
    TrustedInherentInventory { owner: ExternalDefId },
    PrimitiveInherentInventory { owner: crate::PrimitiveType },
    IntrinsicInherentInventory { owner: crate::IntrinsicNominal },
    TrustedValueMemberInventory { owner: ExternalDefId },
}

impl UnavailableCompileInput {
    #[must_use]
    pub const fn capability(&self) -> CompileCapability {
        match self {
            Self::TrustedValueMemberInventory { .. } => {
                CompileCapability::Typecheck(TypecheckCapability::ResolveMembers)
            }
            Self::TrustedInherentInventory { .. }
            | Self::PrimitiveInherentInventory { .. }
            | Self::IntrinsicInherentInventory { .. } => {
                CompileCapability::Typecheck(TypecheckCapability::ResolveTrustedInherentMethods)
            }
        }
    }
}

/// One actual phase-owned capability demand and its semantic provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDemand {
    capability: CompileCapability,
    location: SemanticLocation,
    disposition: DemandDisposition,
    unavailable_input: Option<UnavailableCompileInput>,
}

impl CapabilityDemand {
    #[must_use]
    pub const fn capability(&self) -> CompileCapability {
        self.capability
    }

    #[must_use]
    pub const fn location(&self) -> SemanticLocation {
        self.location
    }

    #[must_use]
    pub const fn disposition(&self) -> DemandDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn unavailable_input(&self) -> Option<&UnavailableCompileInput> {
        self.unavailable_input.as_ref()
    }
}

/// One unavailable input and the exact demand sites requiring it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileInputRefusal {
    input: UnavailableCompileInput,
    locations: Box<[SemanticLocation]>,
}

impl CompileInputRefusal {
    #[must_use]
    pub const fn input(&self) -> &UnavailableCompileInput {
        &self.input
    }

    #[must_use]
    pub fn locations(&self) -> &[SemanticLocation] {
        &self.locations
    }
}

/// One deterministically merged compile-capability observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityTraceEntry {
    capability: CompileCapability,
    disposition: DemandDisposition,
    locations: Box<[SemanticLocation]>,
    input_refusals: Box<[CompileInputRefusal]>,
}

impl CapabilityTraceEntry {
    #[must_use]
    pub const fn capability(&self) -> CompileCapability {
        self.capability
    }

    #[must_use]
    pub const fn disposition(&self) -> DemandDisposition {
        self.disposition
    }

    #[must_use]
    pub fn locations(&self) -> &[SemanticLocation] {
        &self.locations
    }

    #[must_use]
    pub fn input_refusals(&self) -> &[CompileInputRefusal] {
        &self.input_refusals
    }

    #[must_use]
    pub fn is_unsupported(&self) -> bool {
        self.disposition == DemandDisposition::Missing || !self.input_refusals.is_empty()
    }
}

#[derive(Default)]
struct MergedDemand {
    locations: BTreeSet<SemanticLocation>,
    inputs: BTreeMap<UnavailableCompileInput, BTreeSet<SemanticLocation>>,
}

/// Actual phase-owned capability demands merged against one selected support policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityTrace {
    entries: Box<[CapabilityTraceEntry]>,
}

impl CapabilityTrace {
    pub fn from_demands(
        support: &dyn CompileSupport,
        demands: impl IntoIterator<Item = CapabilityDemand>,
    ) -> Result<Self, CapabilityTraceError> {
        let mut merged = BTreeMap::<CompileCapability, (DemandDisposition, MergedDemand)>::new();
        for demand in demands {
            let capability = demand.capability();
            let expected = if support.supports(capability) {
                DemandDisposition::Satisfied
            } else {
                DemandDisposition::Missing
            };
            if demand.disposition() != expected {
                return Err(CapabilityTraceError::SupportMismatch {
                    capability,
                    expected,
                    observed: demand.disposition(),
                });
            }
            let entry = merged
                .entry(capability)
                .or_insert_with(|| (expected, MergedDemand::default()));
            entry.1.locations.insert(demand.location());
            if let Some(input) = demand.unavailable_input {
                entry
                    .1
                    .inputs
                    .entry(input)
                    .or_default()
                    .insert(demand.location);
            }
        }
        let entries = merged
            .into_iter()
            .map(|(capability, (disposition, merged))| CapabilityTraceEntry {
                capability,
                disposition,
                locations: merged.locations.into_iter().collect(),
                input_refusals: merged
                    .inputs
                    .into_iter()
                    .map(|(input, locations)| CompileInputRefusal {
                        input,
                        locations: locations.into_iter().collect(),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Ok(Self { entries })
    }

    #[must_use]
    pub fn entries(&self) -> &[CapabilityTraceEntry] {
        &self.entries
    }

    /// Reports missing profile capabilities, not unavailable catalog inputs.
    #[must_use]
    pub fn has_missing(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.disposition == DemandDisposition::Missing)
    }

    #[must_use]
    pub fn has_unsupported(&self) -> bool {
        self.unsupported().next().is_some()
    }

    pub fn unsupported(&self) -> impl Iterator<Item = &CapabilityTraceEntry> {
        self.entries.iter().filter(|entry| entry.is_unsupported())
    }

    pub fn satisfied(&self) -> impl Iterator<Item = &CapabilityTraceEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.disposition == DemandDisposition::Satisfied)
    }

    pub fn missing(&self) -> impl Iterator<Item = &CapabilityTraceEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.disposition == DemandDisposition::Missing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityTraceError {
    SupportMismatch {
        capability: CompileCapability,
        expected: DemandDisposition,
        observed: DemandDisposition,
    },
}

impl fmt::Display for CapabilityTraceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportMismatch {
                capability,
                expected,
                observed,
            } => write!(
                formatter,
                "capability demand for {} is {observed:?}, but the selected support policy requires {expected:?}",
                capability.stable_key(),
            ),
        }
    }
}

impl std::error::Error for CapabilityTraceError {}

/// Per-phase product paired with the exact capability operations it attempted.
pub struct PhaseAnalysis<T> {
    product: PhaseProduct<T>,
    demands: Box<[CapabilityDemand]>,
}

impl<T> PhaseAnalysis<T> {
    pub(crate) fn new(product: PhaseProduct<T>, demands: Vec<CapabilityDemand>) -> Self {
        Self {
            product,
            demands: demands.into_boxed_slice(),
        }
    }

    #[must_use]
    pub const fn product(&self) -> &PhaseProduct<T> {
        &self.product
    }

    #[must_use]
    pub fn demands(&self) -> &[CapabilityDemand] {
        &self.demands
    }

    #[must_use]
    pub fn into_parts(self) -> (PhaseProduct<T>, Box<[CapabilityDemand]>) {
        (self.product, self.demands)
    }
}

/// Whether a phase established its canonical product or remained unperformed.
pub enum PhaseProduct<T> {
    Performed(T),
    Unperformed,
}

pub(crate) struct DemandRecorder<'support> {
    support: &'support dyn CompileSupport,
    demands: Vec<CapabilityDemand>,
    unavailable_count: usize,
}

impl<'support> DemandRecorder<'support> {
    pub(crate) fn new(support: &'support dyn CompileSupport) -> Self {
        Self {
            support,
            demands: Vec::new(),
            unavailable_count: 0,
        }
    }

    pub(crate) fn supports(&self, capability: CompileCapability) -> bool {
        self.support.supports(capability)
    }

    pub(crate) fn unavailable_count(&self) -> usize {
        self.unavailable_count
    }

    pub(crate) fn require(
        &mut self,
        capability: CompileCapability,
        location: SemanticLocation,
    ) -> bool {
        let supported = self.supports(capability);
        self.unavailable_count += usize::from(!supported);
        self.demands.push(CapabilityDemand {
            capability,
            location,
            disposition: if supported {
                DemandDisposition::Satisfied
            } else {
                DemandDisposition::Missing
            },
            unavailable_input: None,
        });
        supported
    }

    pub(crate) fn refuse_input(
        &mut self,
        input: UnavailableCompileInput,
        location: SemanticLocation,
    ) {
        let capability = input.capability();
        self.unavailable_count += 1;
        let disposition = if self.support.supports(capability) {
            DemandDisposition::Satisfied
        } else {
            DemandDisposition::Missing
        };
        self.demands.push(CapabilityDemand {
            capability,
            location,
            disposition,
            unavailable_input: Some(input),
        });
    }

    pub(crate) fn finish<T>(self, product: PhaseProduct<T>) -> PhaseAnalysis<T> {
        PhaseAnalysis::new(product, self.demands)
    }

    pub(crate) fn into_demands(self) -> Vec<CapabilityDemand> {
        self.demands
    }
}
