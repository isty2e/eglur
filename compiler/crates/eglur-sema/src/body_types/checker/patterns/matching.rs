use super::*;

#[derive(Default)]
struct CarrierBindings {
    enclosing: Option<OriginId>,
    overlap: Option<(OriginId, OriginId)>,
}

impl CarrierBindings {
    fn bind(&mut self, origin: OriginId) {
        if let Some(first) = self.enclosing {
            self.overlap.get_or_insert((first, origin));
        }
    }

    fn enter_alias(&mut self, origin: OriginId) -> Option<OriginId> {
        self.bind(origin);
        self.enclosing.replace(origin)
    }
}

enum PatternPlan<'hir> {
    Complete(Outcome<PatternCheck>),
    Child {
        pattern: PatternId,
        expected: TypeId,
        continuation: PatternContinuation<'hir>,
    },
}

enum PatternContinuation<'hir> {
    Alias {
        binding: LocalBindingId,
        expected: TypeId,
        enclosing: Option<OriginId>,
    },
    Group,
    Tuple {
        cursor: PatternElementCursor<'hir>,
        types: Vec<TypeId>,
        check: PatternCheck,
    },
    Variant {
        cursor: PatternElementCursor<'hir>,
        templates: Vec<TypeId>,
        replacements: BTreeMap<GenericParameterId, TypeId>,
        declaration: CanonicalTarget,
        check: PatternCheck,
    },
}

struct PatternElementCursor<'hir> {
    elements: &'hir [PatternElement],
    next_element: usize,
    next_type: usize,
}

impl<'hir> PatternElementCursor<'hir> {
    fn new(elements: &'hir [PatternElement]) -> Self {
        Self {
            elements,
            next_element: 0,
            next_type: 0,
        }
    }

    fn next(&mut self, type_count: usize) -> Option<(PatternId, usize)> {
        while let Some(element) = self.elements.get(self.next_element) {
            self.next_element += 1;
            match element {
                PatternElement::Pattern(pattern) => {
                    let type_index = self.next_type;
                    self.next_type += 1;
                    return Some((*pattern, type_index));
                }
                PatternElement::Rest(_) => self.next_type = type_count,
            }
        }
        None
    }
}

impl<'input, 'hir, 'support> BodyChecker<'input, 'hir, 'support> {
    pub(in super::super) fn type_match_pattern<'body>(
        &mut self,
        context: BodyContext<'_, 'body>,
        pattern_id: PatternId,
        expected: TypeId,
    ) -> Result<Outcome<PatternCheck>, BodyFlow> {
        context
            .body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        self.pattern_bindings.begin(context.module, pattern_id);
        let mut owners = matches!(
            self.signatures.types().ownership_root(expected)?,
            Some(crate::OwnershipRoot::Optional { .. } | crate::OwnershipRoot::Result { .. })
        )
        .then(CarrierBindings::default);
        let mut pending = (pattern_id, expected);
        let mut continuations = Vec::new();
        loop {
            let mut plan =
                self.prepare_match_pattern(context, pending.0, pending.1, owners.as_mut())?;
            loop {
                match plan {
                    PatternPlan::Child {
                        pattern,
                        expected,
                        continuation,
                    } => {
                        continuations.push(continuation);
                        pending = (pattern, expected);
                        break;
                    }
                    PatternPlan::Complete(outcome) => {
                        let Some(continuation) = continuations.pop() else {
                            if matches!(outcome, Outcome::Known(_))
                                && let Some(poison) = self.match_binding_error(
                                    context,
                                    pattern_id,
                                    owners.and_then(|b| b.overlap),
                                )?
                            {
                                return Ok(Outcome::Poisoned(poison));
                            }
                            return Ok(outcome);
                        };
                        plan = self.resume_match_pattern(continuation, outcome, owners.as_mut())?;
                    }
                }
            }
        }
    }

    fn prepare_match_pattern<'body>(
        &mut self,
        context: BodyContext<'_, 'body>,
        pattern_id: PatternId,
        expected: TypeId,
        owners: Option<&mut CarrierBindings>,
    ) -> Result<PatternPlan<'body>, BodyFlow> {
        let BodyContext {
            module,
            source,
            body,
            ..
        } = context;
        let pattern = body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        match pattern.kind() {
            PatternKind::Binding(name) => {
                if let TypeKind::Nominal {
                    head: NominalHead::Declaration(owner),
                    ..
                } = self.signatures.types().kind(expected)?
                {
                    match self.signatures.enum_variant_names(owner) {
                        FactState::Known(names)
                            if names.contains(
                                &NameKey::from_hir(name).expect("valid binding name"),
                            ) =>
                        {
                            let spelling = name.text().expect("valid binding name");
                            let variant = format!("{}::{spelling}", self.format_type(expected)?);
                            let span = source.origin_span(name.origin()).cloned().ok_or(
                                BodyTypingError::MissingOriginSpan {
                                    file: source.file(),
                                    origin: name.origin(),
                                },
                            )?;
                            let poison = crate::diagnostic::record_name_ambiguous(
                                self.diagnostics,
                                span,
                                spelling,
                                [format!("binding `{spelling}`"), variant],
                            )?;
                            return Ok(PatternPlan::Complete(Outcome::Poisoned(poison)));
                        }
                        FactState::Poisoned(poison) => {
                            return Ok(PatternPlan::Complete(Outcome::Poisoned(poison.clone())));
                        }
                        FactState::Unperformed => {
                            return Ok(PatternPlan::Complete(Outcome::Unperformed));
                        }
                        FactState::Known(_) | FactState::NotApplicable => {}
                    }
                }
                let binding = self.pattern_binding(
                    module,
                    body.id(),
                    crate::resolve::PatternBindingSite::Name(pattern_id),
                    name.origin(),
                )?;
                self.pattern_bindings
                    .observe(binding, pattern_id, name.origin())?;
                self.pattern_bindings.type_binding(binding, expected)?;
                if let Some(owners) = owners
                    && self.signatures.types().ownership_root(expected)?.is_some()
                {
                    owners.bind(name.origin());
                }
                Ok(PatternPlan::Complete(Outcome::Known(
                    PatternCheck::irrefutable().bind_whole(binding),
                )))
            }
            PatternKind::Wildcard => Ok(PatternPlan::Complete(Outcome::Known(
                PatternCheck::irrefutable(),
            ))),
            PatternKind::Alias { name, pattern } => {
                let binding = self.pattern_binding(
                    module,
                    body.id(),
                    crate::resolve::PatternBindingSite::Name(pattern_id),
                    name.origin(),
                )?;
                self.pattern_bindings
                    .observe(binding, pattern_id, name.origin())?;
                let enclosing = match owners {
                    Some(owners) if self.signatures.types().ownership_root(expected)?.is_some() => {
                        owners.enter_alias(name.origin())
                    }
                    Some(owners) => owners.enclosing,
                    None => None,
                };
                Ok(PatternPlan::Child {
                    pattern: *pattern,
                    expected,
                    continuation: PatternContinuation::Alias {
                        binding,
                        expected,
                        enclosing,
                    },
                })
            }
            PatternKind::Group(pattern) => Ok(PatternPlan::Child {
                pattern: *pattern,
                expected,
                continuation: PatternContinuation::Group,
            }),
            PatternKind::Literal(literal) => {
                let actual =
                    self.type_literal(source, literal, TypeRequest::local(Some(expected)))?;
                match actual {
                    Outcome::Known(actual)
                        if self.signatures.types().is_assignable(actual.ty, expected)? =>
                    {
                        Ok(PatternPlan::Complete(Outcome::Known(
                            PatternCheck::partial(),
                        )))
                    }
                    Outcome::Known(actual) => self
                        .pattern_type_mismatch(
                            source,
                            pattern.origin(),
                            "literal",
                            expected,
                            Some(actual.ty),
                        )
                        .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison))),
                    other => Ok(PatternPlan::Complete(
                        other.map(|_| PatternCheck::partial()),
                    )),
                }
            }
            PatternKind::Unit => {
                let unit = self.primitive(PrimitiveType::Unit)?;
                if expected == unit {
                    Ok(PatternPlan::Complete(Outcome::Known(
                        PatternCheck::irrefutable(),
                    )))
                } else {
                    self.pattern_type_mismatch(source, pattern.origin(), "unit", expected, None)
                        .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)))
                }
            }
            PatternKind::Tuple(elements) => {
                self.prepare_tuple_pattern(context, pattern_id, elements, expected)
            }
            PatternKind::Variant { payload, .. } => {
                self.prepare_variant_pattern(context, pattern_id, payload, expected)
            }
            PatternKind::Or(_) | PatternKind::Struct { .. } => {
                Ok(PatternPlan::Complete(self.unsupported(
                    TypecheckCapability::CheckAdvancedPatterns,
                    source,
                    pattern.origin(),
                )))
            }
            PatternKind::Invalid => Ok(PatternPlan::Complete(Outcome::Poisoned(
                source.invalid_poison(pattern.origin()).cloned().ok_or(
                    BodyTypingError::MissingOriginSpan {
                        file: source.file(),
                        origin: pattern.origin(),
                    },
                )?,
            ))),
        }
    }

    fn prepare_tuple_pattern<'body>(
        &mut self,
        context: BodyContext<'_, 'body>,
        pattern_id: PatternId,
        elements: &'body [PatternElement],
        expected: TypeId,
    ) -> Result<PatternPlan<'body>, BodyFlow> {
        let source = context.source;
        let origin = context
            .body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?
            .origin();
        let TypeKind::Tuple(types) = self.signatures.types().kind(expected)? else {
            return self
                .pattern_type_mismatch(source, origin, "tuple", expected, None)
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        };
        let explicit = elements
            .iter()
            .filter(|element| matches!(element, PatternElement::Pattern(_)))
            .count();
        let has_rest = elements
            .iter()
            .any(|element| matches!(element, PatternElement::Rest(_)));
        if (!has_rest && explicit != types.len()) || (has_rest && explicit > types.len()) {
            return self
                .type_error(
                    source,
                    origin,
                    "PATTERN_ARITY_MISMATCH",
                    "tuple pattern arity mismatch",
                    BTreeMap::from([
                        (
                            Box::<str>::from("expected_count"),
                            self.diagnostic_count(types.len())?,
                        ),
                        (
                            Box::<str>::from("actual_count"),
                            self.diagnostic_count(explicit)?,
                        ),
                    ]),
                )
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        }
        let mut cursor = PatternElementCursor::new(elements);
        let Some((pattern, type_index)) = cursor.next(types.len()) else {
            return Ok(PatternPlan::Complete(Outcome::Known(
                PatternCheck::irrefutable(),
            )));
        };
        let types = types[..explicit].to_vec();

        #[cfg(test)]
        super::tuple_tests::observe_copy(types.len());

        Ok(PatternPlan::Child {
            pattern,
            expected: types[type_index],
            continuation: PatternContinuation::Tuple {
                cursor,
                types,
                check: PatternCheck::irrefutable(),
            },
        })
    }

    fn prepare_variant_pattern<'body>(
        &mut self,
        context: BodyContext<'_, 'body>,
        pattern_id: PatternId,
        payload: &'body eglur_hir::PatternPayload,
        expected: TypeId,
    ) -> Result<PatternPlan<'body>, BodyFlow> {
        let source = context.source;
        let pattern = context
            .body
            .pattern(pattern_id)
            .ok_or(BodyTypingError::WrongBodyPattern(pattern_id))?;
        let (target, kind) = match self.pattern_constructor_target(
            context.module,
            source,
            pattern_id,
            pattern.origin(),
        )? {
            Outcome::Known(target) => target,
            Outcome::Poisoned(poison) => {
                return Ok(PatternPlan::Complete(Outcome::Poisoned(poison)));
            }
            Outcome::Deferred => return Ok(PatternPlan::Complete(Outcome::Deferred)),
            Outcome::Unperformed | Outcome::NotApplicable => {
                return Ok(PatternPlan::Complete(Outcome::Unperformed));
            }
        };
        if matches!(payload, eglur_hir::PatternPayload::Named(_)) && kind.is_type_namespace_target()
        {
            return Ok(PatternPlan::Complete(self.unsupported(
                TypecheckCapability::CheckAdvancedPatterns,
                source,
                pattern.origin(),
            )));
        }
        if kind != ResolvedTargetKind::VariantConstructor {
            return self
                .pattern_type_mismatch(source, pattern.origin(), "variant", expected, None)
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        }
        let contract = self.pattern_constructor_contract(&target)?;
        let parameter_set = contract
            .generic_parameters
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut replacements = BTreeMap::new();
        let inferred =
            self.infer_constraints(contract.result, expected, &parameter_set, &mut replacements)?;
        if inferred == ConstraintOutcome::Conflict
            || !contract
                .generic_parameters
                .iter()
                .all(|parameter| replacements.contains_key(parameter))
        {
            return self
                .pattern_type_mismatch(
                    source,
                    pattern.origin(),
                    "variant",
                    expected,
                    Some(contract.result),
                )
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        }
        let result = self
            .signatures
            .types_mut()
            .substitute(contract.result, &replacements)?;
        if result != expected {
            return self
                .pattern_type_mismatch(source, pattern.origin(), "variant", expected, Some(result))
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        }
        let actual_form = ConstructorPayloadForm::from_pattern(payload);
        let expected_form = contract.shape.form();
        if expected_form != actual_form {
            let owner = self.target_label(&contract.declaration);
            let variant = self
                .target_name(&contract.declaration)
                .unwrap_or(owner.as_str())
                .to_owned();
            return self
                .type_error(
                    source,
                    pattern.origin(),
                    "PATTERN_PAYLOAD_FORM_MISMATCH",
                    "pattern payload form mismatch",
                    BTreeMap::from([
                        (Box::<str>::from("owner"), DiagnosticDataValue::from(owner)),
                        (
                            Box::<str>::from("variant"),
                            DiagnosticDataValue::from(variant),
                        ),
                        (
                            Box::<str>::from("expected_form"),
                            DiagnosticDataValue::from(expected_form.spelling()),
                        ),
                        (
                            Box::<str>::from("actual_form"),
                            DiagnosticDataValue::from(actual_form.spelling()),
                        ),
                    ]),
                )
                .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
        }
        match (contract.shape, payload) {
            (PatternConstructorShape::Absent, eglur_hir::PatternPayload::Absent) => {
                Ok(PatternPlan::Complete(Outcome::Known(
                    PatternCheck::variant(contract.declaration, PatternCheck::irrefutable()),
                )))
            }
            (
                PatternConstructorShape::Tuple(arity),
                eglur_hir::PatternPayload::Positional(elements),
            ) => {
                let explicit = elements
                    .iter()
                    .filter(|element| matches!(element, PatternElement::Pattern(_)))
                    .count();
                let has_rest = elements
                    .iter()
                    .any(|element| matches!(element, PatternElement::Rest(_)));
                if (!has_rest && explicit != arity) || (has_rest && explicit > arity) {
                    return self
                        .type_error(
                            source,
                            pattern.origin(),
                            "PATTERN_ARITY_MISMATCH",
                            "variant pattern arity mismatch",
                            BTreeMap::from([
                                (
                                    Box::<str>::from("expected_count"),
                                    self.diagnostic_count(arity)?,
                                ),
                                (
                                    Box::<str>::from("actual_count"),
                                    self.diagnostic_count(explicit)?,
                                ),
                            ]),
                        )
                        .map(|poison| PatternPlan::Complete(Outcome::Poisoned(poison)));
                }
                let mut cursor = PatternElementCursor::new(elements);
                let Some((pattern, type_index)) = cursor.next(arity) else {
                    return Ok(PatternPlan::Complete(Outcome::Known(
                        PatternCheck::variant(contract.declaration, PatternCheck::irrefutable()),
                    )));
                };
                let types = self.pattern_constructor_prefix(&contract.declaration, explicit)?;
                let child_type = self
                    .signatures
                    .types_mut()
                    .substitute(types[type_index], &replacements)?;
                Ok(PatternPlan::Child {
                    pattern,
                    expected: child_type,
                    continuation: PatternContinuation::Variant {
                        cursor,
                        templates: types,
                        replacements,
                        declaration: contract.declaration,
                        check: PatternCheck::irrefutable(),
                    },
                })
            }
            (PatternConstructorShape::Named, eglur_hir::PatternPayload::Named(_)) => {
                Ok(PatternPlan::Complete(self.unsupported(
                    TypecheckCapability::CheckAdvancedPatterns,
                    source,
                    pattern.origin(),
                )))
            }
            _ => Err(BodyFlow::Internal(
                BodyTypingError::InconsistentConstructorPayloadForm {
                    expected: expected_form.spelling(),
                    actual: actual_form.spelling(),
                },
            )),
        }
    }

    fn resume_match_pattern<'body>(
        &mut self,
        continuation: PatternContinuation<'body>,
        outcome: Outcome<PatternCheck>,
        owners: Option<&mut CarrierBindings>,
    ) -> Result<PatternPlan<'body>, BodyFlow> {
        match continuation {
            PatternContinuation::Alias {
                binding,
                expected,
                enclosing,
            } => {
                if let Some(owners) = owners {
                    owners.enclosing = enclosing;
                }
                if let Outcome::Known(_) = &outcome {
                    self.pattern_bindings.type_binding(binding, expected)?;
                }
                Ok(PatternPlan::Complete(
                    outcome.map(|check| check.bind_whole(binding)),
                ))
            }
            PatternContinuation::Group => Ok(PatternPlan::Complete(outcome)),
            PatternContinuation::Tuple {
                mut cursor,
                types,
                mut check,
            } => {
                let Outcome::Known(child) = outcome else {
                    return Ok(PatternPlan::Complete(outcome));
                };
                check.merge(child);

                let Some((pattern, type_index)) = cursor.next(types.len()) else {
                    return Ok(PatternPlan::Complete(Outcome::Known(check)));
                };
                Ok(PatternPlan::Child {
                    pattern,
                    expected: types[type_index],
                    continuation: PatternContinuation::Tuple {
                        cursor,
                        types,
                        check,
                    },
                })
            }
            PatternContinuation::Variant {
                mut cursor,
                templates,
                replacements,
                declaration,
                mut check,
            } => {
                let Outcome::Known(child) = outcome else {
                    return Ok(PatternPlan::Complete(outcome));
                };
                let payload_binding = match child.binding {
                    Some(RootPatternBinding::Whole(id)) if templates.len() == 1 => {
                        Some(RootPatternBinding::Payload(id))
                    }
                    _ => None,
                };
                check.merge(child);

                let Some((pattern, type_index)) = cursor.next(templates.len()) else {
                    let mut variant = PatternCheck::variant(declaration, check);
                    variant.binding = payload_binding;
                    return Ok(PatternPlan::Complete(Outcome::Known(variant)));
                };
                let expected = self
                    .signatures
                    .types_mut()
                    .substitute(templates[type_index], &replacements)?;
                Ok(PatternPlan::Child {
                    pattern,
                    expected,
                    continuation: PatternContinuation::Variant {
                        cursor,
                        templates,
                        replacements,
                        declaration,
                        check,
                    },
                })
            }
        }
    }

    fn pattern_type_mismatch(
        &mut self,
        source: &SemanticSource<'_>,
        origin: OriginId,
        pattern_kind: &'static str,
        expected: TypeId,
        actual: Option<TypeId>,
    ) -> Result<Poison, BodyFlow> {
        let reason = if actual.is_some() {
            "typed_pattern"
        } else {
            "structural_form"
        };
        let mut data = BTreeMap::from([
            (
                Box::<str>::from("reason"),
                DiagnosticDataValue::from(reason),
            ),
            (
                Box::<str>::from("pattern_kind"),
                DiagnosticDataValue::from(pattern_kind),
            ),
            (
                Box::<str>::from("expected_type"),
                DiagnosticDataValue::from(self.format_type(expected)?),
            ),
        ]);
        if let Some(actual) = actual {
            data.insert(
                Box::<str>::from("actual_type"),
                DiagnosticDataValue::from(self.format_type(actual)?),
            );
        }
        self.type_error(
            source,
            origin,
            "PATTERN_TYPE_MISMATCH",
            "pattern type mismatch",
            data,
        )
    }

    pub(in super::super) fn match_domain(
        &mut self,
        scrutinee: TypeId,
    ) -> Result<Option<branching::MatchDomain>, BodyFlow> {
        let payloads = match self.signatures.types().ownership_root(scrutinee)? {
            Some(crate::OwnershipRoot::Optional { payload }) => Some([Some(payload), None]),
            Some(crate::OwnershipRoot::Result { payload, error }) => {
                Some([Some(payload), Some(error)])
            }
            _ => None,
        };
        let mut impossible = [false; 2];
        if let Some(payloads) = payloads {
            if !self.recorder.supports(CompileCapability::Typecheck(
                TypecheckCapability::AnalyzeInhabitance,
            )) {
                return Ok(None);
            }
            for (impossible, payload) in impossible.iter_mut().zip(payloads) {
                if let Some(ty) = payload {
                    *impossible =
                        self.inhabitance.classify(self.signatures, ty)? == Inhabitance::Uninhabited;
                }
            }
        }

        Ok(Some(branching::MatchDomain {
            constructors: self
                .match_variant_universe(scrutinee)?
                .map(|roles| roles.map(Clone::clone)),
            impossible,
            carrier: payloads.is_some(),
        }))
    }

    pub(in super::super) fn check_match_coverage(
        &mut self,
        source: &SemanticSource<'_>,
        domain: &branching::MatchDomain,
        arms: &[eglur_hir::MatchArm],
        patterns: &[PatternCheck],
    ) -> Result<Outcome<Option<Vec<CarrierArm>>>, BodyFlow> {
        assert_eq!(arms.len(), patterns.len(), "all arms passed pattern typing");
        let impossible = domain.impossible;
        let universe = domain.constructors.as_ref().map(|roles| roles.each_ref());
        let mut carrier_arms =
            (domain.carrier && universe.is_some()).then(|| Vec::with_capacity(arms.len()));
        let mut total = universe.is_some() && impossible == [true, true];
        let mut variants = BTreeSet::new();
        if let Some(roles) = universe {
            for (role, impossible) in roles.into_iter().zip(impossible) {
                if impossible {
                    variants.insert(role.clone());
                }
            }
        }

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
                return self
                    .type_error(
                        source,
                        self.pattern_origin(source, arm.pattern)?,
                        "MATCH_UNREACHABLE_ARM",
                        "match arm is unreachable",
                        BTreeMap::new(),
                    )
                    .map(Outcome::Poisoned);
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
        if total {
            return Ok(Outcome::Known(carrier_arms));
        }
        let Some(universe) = universe else {
            let origin = arms
                .first()
                .map(|arm| self.pattern_origin(source, arm.pattern))
                .transpose()?
                .ok_or(BodyTypingError::MissingMatchCoverageOrigin)?;
            self.demand_unimplemented(TypecheckCapability::AnalyzePatternCoverage, source, origin);
            return Ok(Outcome::Unperformed);
        };
        if universe
            .into_iter()
            .all(|variant| variants.contains(variant))
        {
            return Ok(Outcome::Known(carrier_arms));
        }
        let origin = arms
            .first()
            .map(|arm| self.pattern_origin(source, arm.pattern))
            .transpose()?
            .ok_or(BodyTypingError::MissingMatchCoverageOrigin)?;
        self.type_error(
            source,
            origin,
            "MATCH_NON_EXHAUSTIVE",
            "match is non-exhaustive",
            BTreeMap::new(),
        )
        .map(Outcome::Poisoned)
    }

    fn match_variant_universe(
        &self,
        scrutinee: TypeId,
    ) -> Result<Option<[&CanonicalTarget; 2]>, BodyFlow> {
        let TypeKind::Nominal { head, .. } = self.signatures.types().kind(scrutinee)? else {
            return Ok(None);
        };
        let intrinsic = |present, absent| {
            let items = self.signatures.language_items();
            Some([items.declaration(present)?, items.declaration(absent)?])
        };
        let variants = match head {
            NominalHead::Intrinsic(IntrinsicNominal::Opt) => {
                intrinsic(LanguageItem::OptSome, LanguageItem::OptNone)
            }
            NominalHead::Intrinsic(IntrinsicNominal::Res) => {
                intrinsic(LanguageItem::ResOk, LanguageItem::ResErr)
            }
            NominalHead::Declaration(_) => None,
            NominalHead::Intrinsic(IntrinsicNominal::Own | IntrinsicNominal::Box) => None,
        };
        Ok(variants)
    }

    pub(in super::super) fn pattern_origin(
        &self,
        source: &SemanticSource<'_>,
        pattern: PatternId,
    ) -> Result<OriginId, BodyFlow> {
        source
            .hir()
            .and_then(|hir| hir.body(pattern.body()).pattern(pattern))
            .map(eglur_hir::Pattern::origin)
            .ok_or(BodyFlow::Internal(BodyTypingError::WrongBodyPattern(
                pattern,
            )))
    }
}
