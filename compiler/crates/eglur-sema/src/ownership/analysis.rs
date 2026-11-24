use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use eglur_diagnostics::{DiagnosticRootError, DiagnosticStore, DiagnosticStoreError};
use eglur_hir::{Body, FunctionBody, ItemKind, OriginId, PatternId};
use eglur_source::FileId;

use crate::resolve::PatternBindingQueryError;
use crate::{
    BodyTypes, CallableSignature, CallableSignatureId, CompileCapability, CompileSupport,
    DeclarationCollection, DeclarationKind, DeclarationSignature, DeclarationSignatures, DefId,
    DemandRecorder, DuplicateFact, FactState, LocalBindingId, ModuleId, NameResolution,
    PhaseAnalysis, PhaseProduct, SemanticFact, SemanticLocation, SemanticSource, TypeStoreError,
    TypecheckCapability,
};

use super::body::{CoverageRecovery, OwnershipFindings};
use super::{OwnershipFacts, analyze_body, emit_ownership_diagnostics};

/// Performs the bounded ownership phase and emits at most one ownership error per callable.
#[allow(
    clippy::too_many_arguments,
    reason = "the phase boundary keeps independently owned semantic products explicit"
)]
pub fn analyze_ownership(
    collection: &DeclarationCollection,
    names: &NameResolution,
    sources: &[SemanticSource<'_>],
    support: &dyn CompileSupport,
    signatures: &DeclarationSignatures,
    body_types: &BodyTypes,
    diagnostics: &mut DiagnosticStore,
) -> Result<PhaseAnalysis<OwnershipFacts>, OwnershipAnalysisError> {
    let mut recorder = DemandRecorder::new(support);
    let capability = CompileCapability::Typecheck(TypecheckCapability::CheckOwnershipFlow);
    let mut supported = true;
    for module in collection.modules() {
        for declaration in module
            .declarations()
            .filter(|declaration| declaration.kind() == DeclarationKind::Function)
        {
            supported &= recorder.require(
                capability,
                SemanticLocation::origin(module.file(), declaration.origin()),
            );
        }
    }
    if !supported {
        return Ok(recorder.finish(PhaseProduct::Unperformed));
    }

    let (facts, failures) = build_ownership_facts(
        collection,
        names,
        sources,
        signatures,
        body_types,
        &mut recorder,
    )?;

    emit_ownership_diagnostics(&failures, collection, names, sources, diagnostics)?;
    Ok(recorder.finish(PhaseProduct::Performed(facts)))
}

/// Builds function-local ownership facts without repeating name, type, or control resolution.
fn build_ownership_facts(
    collection: &DeclarationCollection,
    names: &NameResolution,
    sources: &[SemanticSource<'_>],
    signatures: &DeclarationSignatures,
    body_types: &BodyTypes,
    recorder: &mut DemandRecorder<'_>,
) -> Result<(OwnershipFacts, Vec<OwnershipFindings>), OwnershipAnalysisError> {
    let mut source_map = BTreeMap::new();
    for source in sources {
        if source_map.insert(source.file(), source).is_some() {
            return Err(OwnershipAnalysisError::DuplicateSource(source.file()));
        }
    }
    let mut facts = OwnershipFacts::new();
    let mut failures = Vec::new();
    for declaration in collection
        .modules()
        .flat_map(|module| module.declarations())
    {
        if declaration.kind() != DeclarationKind::Function {
            continue;
        }
        let signature_id = match signatures.source_declaration(declaration.id()) {
            FactState::Known(DeclarationSignature::Callable(signature_id)) => *signature_id,
            FactState::Known(_) => {
                return Err(OwnershipAnalysisError::WrongSignatureKind(declaration.id()));
            }
            FactState::Poisoned(poison) => {
                facts
                    .bodies
                    .insert(declaration.id(), SemanticFact::Poisoned(poison.clone()))?;
                continue;
            }
            FactState::Unperformed => {
                facts.complete = false;
                continue;
            }
            FactState::NotApplicable => {
                facts
                    .bodies
                    .insert(declaration.id(), SemanticFact::NotApplicable)?;
                continue;
            }
        };
        let signature = signatures.callable(signature_id).ok_or(
            OwnershipAnalysisError::UnknownCallableSignature(signature_id),
        )?;
        if signature.body().is_none() {
            continue;
        }
        let rejected_matches = body_types.rejected_coverage.body(declaration.id());
        let recovery = match (body_types.body(declaration.id()), rejected_matches) {
            (FactState::Known(_), _) => None,
            (FactState::Poisoned(poison), Some(sites)) => {
                Some(CoverageRecovery::new(poison, sites)?)
            }
            (FactState::Poisoned(poison), None) => {
                facts
                    .bodies
                    .insert(declaration.id(), SemanticFact::Poisoned(poison.clone()))?;
                continue;
            }
            (FactState::Unperformed, _) => {
                facts.complete = false;
                continue;
            }
            (FactState::NotApplicable, _) => {
                facts
                    .bodies
                    .insert(declaration.id(), SemanticFact::NotApplicable)?;
                continue;
            }
        };

        let module = collection.module(declaration.id().module()).ok_or(
            OwnershipAnalysisError::UnknownModule(declaration.id().module()),
        )?;
        let source = source_map
            .get(&module.file())
            .copied()
            .ok_or(OwnershipAnalysisError::MissingSource(module.file()))?;
        let hir = source
            .hir()
            .ok_or(OwnershipAnalysisError::MissingHir(module.file()))?;
        let item_id = declaration
            .item()
            .ok_or(OwnershipAnalysisError::MissingDeclarationItem(
                declaration.id(),
            ))?;
        let ItemKind::Function(function) = hir.item(item_id).kind() else {
            return Err(OwnershipAnalysisError::WrongDeclarationKind(
                declaration.id(),
            ));
        };
        let root = match function.body_form {
            FunctionBody::Absent => continue,
            FunctionBody::Expression(root) | FunctionBody::Block(root) => root,
        };
        let body = hir.body(function.body);
        let parameters = parameter_bindings(names, module.id(), body, signature)?;
        let analysis = analyze_body(
            declaration.id(),
            module.id(),
            body,
            root,
            parameters,
            names,
            signatures,
            body_types,
            recovery,
        )?;
        match analysis.findings {
            Some(found) if !found.failures.is_empty() => failures.push(found),
            Some(_) => {}
            None => {
                facts.complete = false;
                recorder.require(
                    CompileCapability::Typecheck(TypecheckCapability::CheckCompleteOwnershipFlow),
                    SemanticLocation::origin(module.file(), declaration.origin()),
                );
            }
        }
        facts.bodies.insert(declaration.id(), analysis.fact)?;
    }
    Ok((facts, failures))
}

fn parameter_bindings(
    names: &NameResolution,
    module: ModuleId,
    body: &Body,
    signature: &CallableSignature,
) -> Result<BTreeSet<LocalBindingId>, OwnershipAnalysisError> {
    let mut bindings = BTreeSet::new();
    for parameter in signature.parameters() {
        if let Some(pattern) = parameter.source_pattern() {
            bindings.extend(names.pattern_bindings(module, body, pattern)?);
        }
    }
    Ok(bindings)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnershipAnalysisError {
    DuplicateSource(FileId),
    UnknownModule(ModuleId),
    MissingSource(FileId),
    MissingHir(FileId),
    MissingDeclarationItem(DefId),
    WrongDeclarationKind(DefId),
    WrongSignatureKind(DefId),
    WrongBodyExpression(eglur_hir::ExprId),
    WrongBodyStatement(eglur_hir::StmtId),
    WrongBodyPattern(PatternId),
    MissingLocalOrigin {
        module: ModuleId,
        body: eglur_hir::BodyId,
        origin: OriginId,
    },
    MissingLocalType(LocalBindingId),
    MissingExpressionType(crate::ExpressionKey),
    InvalidTupleProjectionIndex {
        expression: crate::ExpressionKey,
        index: Box<str>,
    },
    MissingOriginSpan {
        file: FileId,
        origin: OriginId,
    },
    InvalidCompletionSpan {
        file: FileId,
        origin: OriginId,
    },
    UnknownCallableSignature(CallableSignatureId),
    TypeStore(TypeStoreError),
    DuplicateFact,
    DiagnosticRoot(DiagnosticRootError),
    DiagnosticStore(DiagnosticStoreError),
}

impl fmt::Display for OwnershipAnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ownership analysis invariant failed: {self:?}")
    }
}

impl std::error::Error for OwnershipAnalysisError {}

impl From<TypeStoreError> for OwnershipAnalysisError {
    fn from(error: TypeStoreError) -> Self {
        Self::TypeStore(error)
    }
}

impl From<PatternBindingQueryError> for OwnershipAnalysisError {
    fn from(error: PatternBindingQueryError) -> Self {
        match error {
            PatternBindingQueryError::WrongBodyPattern(pattern) => Self::WrongBodyPattern(pattern),
            PatternBindingQueryError::MissingLocalOrigin {
                module,
                body,
                origin,
            } => Self::MissingLocalOrigin {
                module,
                body,
                origin,
            },
        }
    }
}

impl From<DuplicateFact> for OwnershipAnalysisError {
    fn from(_: DuplicateFact) -> Self {
        Self::DuplicateFact
    }
}

impl From<crate::diagnostic::SemanticDiagnosticError> for OwnershipAnalysisError {
    fn from(error: crate::diagnostic::SemanticDiagnosticError) -> Self {
        match error {
            crate::diagnostic::SemanticDiagnosticError::Root(error) => Self::DiagnosticRoot(error),
            crate::diagnostic::SemanticDiagnosticError::Store(error) => {
                Self::DiagnosticStore(error)
            }
        }
    }
}
