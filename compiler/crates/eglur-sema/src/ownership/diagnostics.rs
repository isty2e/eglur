use std::collections::BTreeMap;

use eglur_diagnostics::{DiagnosticDataValue, DiagnosticStore, RelatedSpan, SourceSpan};
use eglur_source::{ByteOffset, ByteRange, FileSpan};

use crate::diagnostic::record_type_error_with_related;
use crate::{DeclarationCollection, LocalBindingId, NameResolution, SemanticSource};

use super::model::OwnershipCause;
use super::{OwnershipAnalysisError, OwnershipFailure, OwnershipFailureKind};

pub(crate) fn emit_ownership_diagnostics(
    failures: &[OwnershipFailure],
    collection: &DeclarationCollection,
    names: &NameResolution,
    sources: &[SemanticSource<'_>],
    diagnostics: &mut DiagnosticStore,
) -> Result<(), OwnershipAnalysisError> {
    let mut source_map = BTreeMap::new();
    for source in sources {
        if source_map.insert(source.file(), source).is_some() {
            return Err(OwnershipAnalysisError::DuplicateSource(source.file()));
        }
    }
    let local_names = names
        .locals()
        .iter()
        .map(|binding| (binding.id(), binding.name().spelling()))
        .collect::<BTreeMap<_, _>>();
    let mut by_owner = BTreeMap::new();
    for failure in failures {
        by_owner
            .entry(failure.owner())
            .or_insert_with(Vec::new)
            .push(resolve_failure(
                failure,
                collection,
                &source_map,
                &local_names,
            )?);
    }
    for candidates in by_owner.values_mut() {
        candidates.sort_by(|left, right| left.order_key().cmp(&right.order_key()));
        let Some(selected) = candidates.first() else {
            continue;
        };
        let related = selected.related.clone().into_iter().collect::<Vec<_>>();
        record_type_error_with_related(
            diagnostics,
            selected.code,
            selected.message.clone(),
            selected.span.clone(),
            related,
            BTreeMap::<Box<str>, DiagnosticDataValue>::new(),
        )?;
    }
    Ok(())
}

struct ResolvedFailure {
    code: &'static str,
    message: String,
    span: SourceSpan,
    related: Option<RelatedSpan>,
}

impl ResolvedFailure {
    fn order_key(
        &self,
    ) -> (
        &str,
        eglur_source::ByteOffset,
        eglur_source::ByteOffset,
        &'static str,
    ) {
        (
            self.span.source().as_str(),
            self.span.file_span().range().start(),
            self.span.file_span().range().end(),
            self.code,
        )
    }
}

fn resolve_failure(
    failure: &OwnershipFailure,
    collection: &DeclarationCollection,
    sources: &BTreeMap<eglur_source::FileId, &SemanticSource<'_>>,
    local_names: &BTreeMap<LocalBindingId, String>,
) -> Result<ResolvedFailure, OwnershipAnalysisError> {
    let module = collection.module(failure.owner().module()).ok_or(
        OwnershipAnalysisError::UnknownModule(failure.owner().module()),
    )?;
    let source = sources
        .get(&module.file())
        .copied()
        .ok_or(OwnershipAnalysisError::MissingSource(module.file()))?;
    let span = source.origin_span(failure.origin()).cloned().ok_or(
        OwnershipAnalysisError::MissingOriginSpan {
            file: source.file(),
            origin: failure.origin(),
        },
    )?;
    let span = if failure.closing_delimiter() {
        closing_delimiter_span(span, failure.origin())?
    } else {
        span
    };
    let (code, message, related_site) = match failure.cause() {
        OwnershipCause::Binding { kind, binding, first_consume } => {
            let name = local_names.get(&binding).map_or("<ownership root>", String::as_str);
            let (code, message) = binding_diagnostic(kind, name);
            let related = first_consume.map(|origin| (origin, "ownership value first consumed here"));
            (code, message, related)
        }
        OwnershipCause::PendingMatchExit { scrutinee } => (
            "OWNERSHIP_PATH_UNBALANCED",
            "this guard exit leaves a possible ownership payload without discharging it exactly once on this path".to_owned(),
            Some((scrutinee, "this match may still hold an ownership payload")),
        ),
        OwnershipCause::UnboundMatchPayload { scrutinee } => (
            "OWNERSHIP_DISCARD_FORBIDDEN",
            "ownership discard is forbidden: this arm can select a payload without binding it or its carrier".to_owned(),
            Some((scrutinee, "ownership value matched here")),
        ),
    };
    let related = if let Some((origin, label)) =
        related_site.filter(|(origin, _)| *origin != failure.origin())
    {
        let related = source.origin_span(origin).cloned().ok_or(
            OwnershipAnalysisError::MissingOriginSpan {
                file: source.file(),
                origin,
            },
        )?;
        Some(RelatedSpan::new(related, label).expect("static related-span label is non-empty"))
    } else {
        None
    };
    Ok(ResolvedFailure {
        code,
        message,
        span,
        related,
    })
}

fn binding_diagnostic(kind: OwnershipFailureKind, name: &str) -> (&'static str, String) {
    match kind {
        OwnershipFailureKind::TransferRequiresMove => (
            "OWNERSHIP_TRANSFER_REQUIRES_MOVE",
            format!("ownership transfer of `{name}` requires move"),
        ),
        OwnershipFailureKind::DoubleMove => (
            "OWNERSHIP_DOUBLE_MOVE",
            format!("ownership move is already consumed for `{name}` on this path"),
        ),
        OwnershipFailureKind::UseAfterDischarge => (
            "OWNERSHIP_USE_AFTER_DISCHARGE",
            format!("ownership use references discharged root `{name}`"),
        ),
        OwnershipFailureKind::PathUnbalanced => (
            "OWNERSHIP_PATH_UNBALANCED",
            format!(
                "ownership path for `{name}` is not discharged exactly once before its obligation region exits"
            ),
        ),
        OwnershipFailureKind::GuardConsume => (
            "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
            format!("ownership guard: forbidden to consume tentative binding `{name}`"),
        ),
    }
}

fn closing_delimiter_span(
    span: SourceSpan,
    origin: eglur_hir::OriginId,
) -> Result<SourceSpan, OwnershipAnalysisError> {
    let file_span = span.file_span();
    let end = file_span.range().end();
    let Some(start) = end.get().checked_sub(1) else {
        return Err(OwnershipAnalysisError::InvalidCompletionSpan {
            file: file_span.file(),
            origin,
        });
    };
    let range = ByteRange::new(ByteOffset::new(start), end).ok_or(
        OwnershipAnalysisError::InvalidCompletionSpan {
            file: file_span.file(),
            origin,
        },
    )?;
    Ok(SourceSpan::byte_only(
        span.source().clone(),
        FileSpan::new(file_span.file(), range),
    ))
}
