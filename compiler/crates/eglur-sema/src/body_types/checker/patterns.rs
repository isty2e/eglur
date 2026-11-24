use super::*;

mod bindings;
mod construction;
mod matching;

pub(super) use bindings::PatternBindingScratch;
use construction::PatternConstructorShape;

#[cfg(test)]
mod construction_tests;

#[cfg(test)]
mod poison_tests;

#[cfg(test)]
mod tuple_tests;

impl<'input, 'hir, 'support> BodyChecker<'input, 'hir, 'support> {
    fn match_binding_error(
        &mut self,
        context: BodyContext<'_, '_>,
        root: PatternId,
        overlap: Option<(OriginId, OriginId)>,
    ) -> Result<Option<Poison>, BodyFlow> {
        let source = context.source;
        if let Some(duplicate) = self.pattern_bindings.duplicate(context.module, root) {
            let pattern = context
                .body
                .pattern(duplicate.pattern)
                .ok_or(BodyTypingError::WrongBodyPattern(duplicate.pattern))?;
            let name = match pattern.kind() {
                PatternKind::Binding(name) | PatternKind::Alias { name, .. } => name,
                _ => {
                    return Err(
                        BodyTypingError::InconsistentPatternBinding(duplicate.binding).into(),
                    );
                }
            };
            if let Some(name) = NameKey::from_hir(name) {
                return self
                    .type_error(
                        source,
                        duplicate.origin,
                        "PATTERN_DUPLICATE_BINDING",
                        format!("pattern binding `{}` is duplicated", name.spelling()),
                        BTreeMap::from([(
                            Box::<str>::from("name"),
                            DiagnosticDataValue::from(name.spelling()),
                        )]),
                    )
                    .map(Some);
            }
        }
        if let Some((first, later)) = overlap {
            let span =
                source
                    .origin_span(first)
                    .cloned()
                    .ok_or(BodyTypingError::MissingOriginSpan {
                        file: source.file(),
                        origin: first,
                    })?;
            return self
                .type_error_with_related(
                    source,
                    later,
                    "PATTERN_OWNERSHIP_BINDING_OVERLAP",
                    "overlapping ownership bindings in pattern: this root is already bound",
                    RelatedSpan::new(span, "first ownership binding here"),
                    BTreeMap::new(),
                )
                .map(Some);
        }
        Ok(None)
    }

    pub(super) fn install_match_bindings(
        &mut self,
        module: ModuleId,
        root: PatternId,
    ) -> Result<(), BodyFlow> {
        let names = self.names;
        let view = names.body_bindings(module, root.body());
        let bindings = view.as_ref().and_then(|view| view.root(root));
        if let Some(bindings) = bindings {
            for &binding in bindings {
                let ty = self.pattern_bindings.ty(binding)?;
                self.bind_match_local(binding, ty)?;
            }
        } else if let Some(binding) = self.pattern_bindings.first_binding(module, root) {
            return Err(BodyTypingError::InconsistentPatternBinding(binding).into());
        }
        Ok(())
    }

    pub(super) fn bind_match_local(
        &mut self,
        binding: LocalBindingId,
        ty: TypeId,
    ) -> Result<(), BodyFlow> {
        let region = self.current_control_region()?.clone();
        match self.locals.state(&binding) {
            FactState::Known(existing) if existing.ty == ty && existing.region == region => Ok(()),
            FactState::Unperformed => self
                .locals
                .insert(
                    binding,
                    SemanticFact::Known(LocalTypeFact {
                        ty,
                        mutability: Mutability::Immutable,
                        region,
                    }),
                )
                .map_err(Into::into),
            FactState::Known(_) | FactState::Poisoned(_) | FactState::NotApplicable => Err(
                BodyFlow::Internal(BodyTypingError::InconsistentPatternBinding(binding)),
            ),
        }
    }
    pub(super) fn unsupported_let_pattern(
        body: &Body,
        pattern_id: PatternId,
    ) -> Result<Option<OriginId>, BodyFlow> {
        let pattern = body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        match pattern.kind() {
            PatternKind::Binding(_)
            | PatternKind::Wildcard
            | PatternKind::Unit
            | PatternKind::Invalid => Ok(None),
            PatternKind::Group(inner) => Self::unsupported_let_pattern(body, *inner),
            PatternKind::Tuple(elements) => {
                for element in elements {
                    let PatternElement::Pattern(inner) = element else {
                        return Ok(Some(pattern.origin()));
                    };
                    if let Some(origin) = Self::unsupported_let_pattern(body, *inner)? {
                        return Ok(Some(origin));
                    }
                }
                Ok(None)
            }
            _ => Ok(Some(pattern.origin())),
        }
    }

    pub(super) fn first_pattern_duplicate(
        &self,
        body: &Body,
        pattern: PatternId,
    ) -> Result<Option<(NameKey, OriginId)>, BodyFlow> {
        let mut seen = BTreeMap::new();
        self.first_pattern_duplicate_with(body, pattern, &mut seen)
    }

    pub(super) fn first_pattern_duplicate_with(
        &self,
        body: &Body,
        pattern_id: PatternId,
        seen: &mut BTreeMap<NameKey, OriginId>,
    ) -> Result<Option<(NameKey, OriginId)>, BodyFlow> {
        let pattern = body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        match pattern.kind() {
            PatternKind::Binding(name) => Ok(Self::duplicate_pattern_name(name, seen)),
            PatternKind::Alias { name, pattern } => {
                if let Some(duplicate) = Self::duplicate_pattern_name(name, seen) {
                    return Ok(Some(duplicate));
                }
                self.first_pattern_duplicate_with(body, *pattern, seen)
            }
            PatternKind::Group(pattern) => self.first_pattern_duplicate_with(body, *pattern, seen),
            PatternKind::Tuple(elements) => {
                for element in elements {
                    if let PatternElement::Pattern(pattern) = element
                        && let Some(duplicate) =
                            self.first_pattern_duplicate_with(body, *pattern, seen)?
                    {
                        return Ok(Some(duplicate));
                    }
                }
                Ok(None)
            }
            PatternKind::Variant { payload, .. } => match payload {
                eglur_hir::PatternPayload::Positional(elements) => {
                    for element in elements {
                        if let PatternElement::Pattern(pattern) = element
                            && let Some(duplicate) =
                                self.first_pattern_duplicate_with(body, *pattern, seen)?
                        {
                            return Ok(Some(duplicate));
                        }
                    }
                    Ok(None)
                }
                eglur_hir::PatternPayload::Named(fields) => {
                    self.first_field_pattern_duplicate(body, fields, seen)
                }
                eglur_hir::PatternPayload::Absent => Ok(None),
            },
            PatternKind::Struct { fields, .. } => {
                self.first_field_pattern_duplicate(body, fields, seen)
            }
            PatternKind::Or(alternatives) => {
                let base = seen.clone();
                for alternative in alternatives {
                    let mut alternative_seen = base.clone();
                    if let Some(duplicate) = self.first_pattern_duplicate_with(
                        body,
                        *alternative,
                        &mut alternative_seen,
                    )? {
                        return Ok(Some(duplicate));
                    }
                }
                Ok(None)
            }
            PatternKind::Wildcard
            | PatternKind::Literal(_)
            | PatternKind::Unit
            | PatternKind::Invalid => Ok(None),
        }
    }

    pub(super) fn first_field_pattern_duplicate(
        &self,
        body: &Body,
        fields: &[eglur_hir::FieldPattern],
        seen: &mut BTreeMap<NameKey, OriginId>,
    ) -> Result<Option<(NameKey, OriginId)>, BodyFlow> {
        for field in fields {
            match field {
                eglur_hir::FieldPattern::Field {
                    name,
                    pattern: None,
                    ..
                } => {
                    if let Some(duplicate) = Self::duplicate_pattern_name(name, seen) {
                        return Ok(Some(duplicate));
                    }
                }
                eglur_hir::FieldPattern::Field {
                    pattern: Some(pattern),
                    ..
                } => {
                    if let Some(duplicate) =
                        self.first_pattern_duplicate_with(body, *pattern, seen)?
                    {
                        return Ok(Some(duplicate));
                    }
                }
                eglur_hir::FieldPattern::Rest(_) => {}
            }
        }
        Ok(None)
    }

    pub(super) fn duplicate_pattern_name(
        name: &Name,
        seen: &mut BTreeMap<NameKey, OriginId>,
    ) -> Option<(NameKey, OriginId)> {
        let name_key = NameKey::from_hir(name)?;
        if seen.insert(name_key.clone(), name.origin()).is_some() {
            Some((name_key, name.origin()))
        } else {
            None
        }
    }

    pub(super) fn bind_pattern(
        &mut self,
        module: ModuleId,
        source: &SemanticSource<'_>,
        body: &Body,
        pattern_id: PatternId,
        ty: TypeId,
        mode: PatternBindingMode,
    ) -> Result<Outcome<()>, BodyFlow> {
        let mutability = mode.mutability();
        let mut binding_pattern = pattern_id;
        let mut pattern = body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;

        if matches!(mode, PatternBindingMode::Let(_)) {
            while let PatternKind::Group(inner) = pattern.kind() {
                binding_pattern = *inner;
                pattern = body
                    .pattern(*inner)
                    .ok_or(BodyTypingError::WrongBodyPattern(*inner))?;
            }
        }

        let origin = pattern.origin();
        let kind = pattern.kind().clone();
        match kind {
            PatternKind::Binding(name) => {
                let binding = self.pattern_binding(
                    module,
                    body.id(),
                    crate::resolve::PatternBindingSite::Name(binding_pattern),
                    name.origin(),
                )?;
                let region = self.current_control_region()?.clone();
                self.locals.insert(
                    binding,
                    SemanticFact::Known(LocalTypeFact {
                        ty,
                        mutability,
                        region,
                    }),
                )?;
                Ok(Outcome::Known(()))
            }
            PatternKind::Wildcard => {
                if self.signatures.types().ownership_root(ty)?.is_some() {
                    self.type_error(
                        source,
                        origin,
                        "OWNERSHIP_DISCARD_FORBIDDEN",
                        "ownership value discard is forbidden",
                        BTreeMap::new(),
                    )
                    .map(Outcome::Poisoned)
                } else {
                    Ok(Outcome::Known(()))
                }
            }
            PatternKind::Unit if matches!(mode, PatternBindingMode::Let(_)) => {
                let unit = self.primitive(PrimitiveType::Unit)?;
                if ty == unit {
                    Ok(Outcome::Known(()))
                } else {
                    let data = BTreeMap::from([
                        (
                            Box::<str>::from("reason"),
                            DiagnosticDataValue::from("unit_required"),
                        ),
                        (
                            Box::<str>::from("pattern_kind"),
                            DiagnosticDataValue::from("unit"),
                        ),
                        (
                            Box::<str>::from("actual_type"),
                            DiagnosticDataValue::from(self.format_type(ty)?),
                        ),
                        (
                            Box::<str>::from("expected_type"),
                            DiagnosticDataValue::from("Unit"),
                        ),
                    ]);
                    self.type_error(
                        source,
                        origin,
                        "LET_PATTERN_TYPE_MISMATCH",
                        "let Unit pattern type mismatch",
                        data,
                    )
                    .map(Outcome::Poisoned)
                }
            }
            PatternKind::Tuple(elements) if matches!(mode, PatternBindingMode::Let(_)) => {
                if elements
                    .iter()
                    .any(|element| matches!(element, PatternElement::Rest(_)))
                {
                    return self
                        .type_error(
                            source,
                            origin,
                            "LET_PATTERN_UNSUPPORTED",
                            "let pattern is unsupported in v0.1",
                            BTreeMap::new(),
                        )
                        .map(Outcome::Poisoned);
                }
                let TypeKind::Tuple(types) = self.signatures.types().kind(ty)?.clone() else {
                    let data = BTreeMap::from([
                        (
                            Box::<str>::from("reason"),
                            DiagnosticDataValue::from("tuple_required"),
                        ),
                        (
                            Box::<str>::from("pattern_kind"),
                            DiagnosticDataValue::from("tuple"),
                        ),
                        (
                            Box::<str>::from("actual_type"),
                            DiagnosticDataValue::from(self.format_type(ty)?),
                        ),
                    ]);
                    return self
                        .type_error(
                            source,
                            origin,
                            "LET_PATTERN_TYPE_MISMATCH",
                            "let tuple pattern type mismatch: initializer is not a tuple",
                            data,
                        )
                        .map(Outcome::Poisoned);
                };
                if elements.len() != types.len() {
                    let expected_count = self.diagnostic_count(elements.len())?;
                    let actual_count = self.diagnostic_count(types.len())?;
                    let data = BTreeMap::from([
                        (
                            Box::<str>::from("reason"),
                            DiagnosticDataValue::from("tuple_arity"),
                        ),
                        (
                            Box::<str>::from("pattern_kind"),
                            DiagnosticDataValue::from("tuple"),
                        ),
                        (
                            Box::<str>::from("actual_type"),
                            DiagnosticDataValue::from(self.format_type(ty)?),
                        ),
                        (Box::<str>::from("expected_count"), expected_count),
                        (Box::<str>::from("actual_count"), actual_count),
                    ]);
                    return self
                        .type_error(
                            source,
                            origin,
                            "LET_PATTERN_TYPE_MISMATCH",
                            "let tuple pattern type mismatch: arity differs",
                            data,
                        )
                        .map(Outcome::Poisoned);
                }
                let mut outcome = UnitOutcomeFold::Known;
                for (element, element_ty) in elements.iter().zip(types.iter().copied()) {
                    let PatternElement::Pattern(element) = element else {
                        unreachable!("rest elements were rejected before binding")
                    };
                    let bound =
                        self.bind_pattern(module, source, body, *element, element_ty, mode)?;
                    outcome = outcome.include(bound)?;
                }
                Ok(outcome.finish())
            }
            PatternKind::Invalid => Ok(Outcome::Poisoned(
                source.invalid_poison(origin).cloned().ok_or(
                    BodyTypingError::MissingOriginSpan {
                        file: source.file(),
                        origin,
                    },
                )?,
            )),
            _ => {
                let (code, message) = match mode {
                    PatternBindingMode::Parameter => (
                        "PARAM_PATTERN_UNSUPPORTED",
                        "parameter pattern is unsupported in v0.1",
                    ),
                    PatternBindingMode::Let(_) => (
                        "LET_PATTERN_UNSUPPORTED",
                        "let pattern is unsupported in v0.1",
                    ),
                };
                self.type_error(source, origin, code, message, BTreeMap::new())
                    .map(Outcome::Poisoned)
            }
        }
    }

    pub(super) fn bind_pattern_poison(
        &mut self,
        module: ModuleId,
        body: &Body,
        pattern_id: PatternId,
        poison: Poison,
    ) -> Result<(), BodyFlow> {
        let pattern = body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        match pattern.kind() {
            PatternKind::Binding(name) => {
                let binding = self.pattern_binding(
                    module,
                    body.id(),
                    crate::resolve::PatternBindingSite::Name(pattern_id),
                    name.origin(),
                )?;
                self.poison_pattern_local(binding, poison)?;
            }
            PatternKind::Alias { name, pattern } => {
                let binding = self.pattern_binding(
                    module,
                    body.id(),
                    crate::resolve::PatternBindingSite::Name(pattern_id),
                    name.origin(),
                )?;
                self.poison_pattern_local(binding, poison.clone())?;
                self.bind_pattern_poison(module, body, *pattern, poison)?;
            }
            PatternKind::Or(alternatives) => {
                for alternative in alternatives {
                    self.bind_pattern_poison(module, body, *alternative, poison.clone())?;
                }
            }
            PatternKind::Tuple(alternatives) => {
                for alternative in alternatives {
                    if let PatternElement::Pattern(pattern) = alternative {
                        self.bind_pattern_poison(module, body, *pattern, poison.clone())?;
                    }
                }
            }
            PatternKind::Group(inner) => {
                self.bind_pattern_poison(module, body, *inner, poison)?;
            }
            PatternKind::Struct { fields, .. } => {
                for (field_index, field) in fields.iter().enumerate() {
                    match field {
                        eglur_hir::FieldPattern::Field {
                            name,
                            pattern: None,
                            ..
                        } => {
                            let binding = self.pattern_binding(
                                module,
                                body.id(),
                                crate::resolve::PatternBindingSite::Shorthand {
                                    pattern: pattern_id,
                                    field: field_index,
                                },
                                name.origin(),
                            )?;
                            self.poison_pattern_local(binding, poison.clone())?;
                        }
                        eglur_hir::FieldPattern::Field {
                            pattern: Some(pattern),
                            ..
                        } => {
                            self.bind_pattern_poison(module, body, *pattern, poison.clone())?;
                        }
                        eglur_hir::FieldPattern::Rest(_) => {}
                    }
                }
            }
            PatternKind::Variant { payload, .. } => match payload {
                eglur_hir::PatternPayload::Positional(elements) => {
                    for element in elements {
                        if let PatternElement::Pattern(pattern) = element {
                            self.bind_pattern_poison(module, body, *pattern, poison.clone())?;
                        }
                    }
                }
                eglur_hir::PatternPayload::Named(fields) => {
                    for (field_index, field) in fields.iter().enumerate() {
                        match field {
                            eglur_hir::FieldPattern::Field {
                                name,
                                pattern: None,
                                ..
                            } => {
                                let binding = self.pattern_binding(
                                    module,
                                    body.id(),
                                    crate::resolve::PatternBindingSite::Shorthand {
                                        pattern: pattern_id,
                                        field: field_index,
                                    },
                                    name.origin(),
                                )?;
                                self.poison_pattern_local(binding, poison.clone())?;
                            }
                            eglur_hir::FieldPattern::Field {
                                pattern: Some(pattern),
                                ..
                            } => {
                                self.bind_pattern_poison(module, body, *pattern, poison.clone())?;
                            }
                            eglur_hir::FieldPattern::Rest(_) => {}
                        }
                    }
                }
                eglur_hir::PatternPayload::Absent => {}
            },
            PatternKind::Wildcard
            | PatternKind::Literal(_)
            | PatternKind::Unit
            | PatternKind::Invalid => {}
        }
        Ok(())
    }

    pub(super) fn poison_pattern_local(
        &mut self,
        binding: LocalBindingId,
        poison: Poison,
    ) -> Result<(), BodyFlow> {
        match self.locals.state(&binding) {
            FactState::Unperformed => self
                .locals
                .insert(binding, SemanticFact::Poisoned(poison))
                .map_err(Into::into),
            FactState::Poisoned(_) => Ok(()),
            FactState::Known(_) | FactState::NotApplicable => Err(BodyFlow::Internal(
                BodyTypingError::InconsistentPatternBinding(binding),
            )),
        }
    }
}
