use super::*;

impl<'input, 'hir, 'support> BodyChecker<'input, 'hir, 'support> {
    pub(super) fn finish(
        mut self,
        _: execution::CompletedBodyTraversal,
    ) -> Result<CompletedBodyExecution, BodyTypingError> {
        #[cfg(test)]
        execution::tests::sweep_support::finished();
        self.transfers.sort();
        self.transfers.dedup();
        debug_assert!(self.region_stack.is_empty());
        debug_assert!(self.block_region_overrides.is_empty());
        debug_assert!(self.result_contexts.is_empty());
        debug_assert_eq!(self.question_floors, [0]);
        debug_assert!(self.checked_contexts.is_empty());
        debug_assert!(self.control_path_barriers.is_empty());
        let analysis = self.recorder.finish(PhaseProduct::Performed(BodyTypes {
            expressions: self.expressions,
            locals: self.locals,
            members: self.members,
            invocations: self.invocations,
            callable_values: self.callable_values,
            reduce_by_commitments: self.reduce_by_commitments,
            regions: self.regions,
            checked_numeric: self.checked_numeric,
            ownership_operations: self.ownership_operations,
            match_arms: self.match_arms,
            rejected_coverage: self.rejected_coverage,
            bodies: self.bodies,
            transfers: self.transfers.into_boxed_slice(),
            callable_instances: self.callable_instances.into_boxed_slice(),
            obligations: self.obligations.into_boxed_slice(),
            method_dispatches: self.method_dispatches.into_boxed_slice(),
        }));
        CompletedBodyExecution::new(analysis, self.proof_inputs)
    }

    pub(super) fn check_all(&mut self) -> Result<(), BodyFlow> {
        self.ensure_analysis_active()?;
        let declarations = self
            .collection
            .modules()
            .flat_map(|module| module.declarations())
            .map(|declaration| declaration.id())
            .collect::<Vec<_>>();
        let constants = declarations
            .iter()
            .copied()
            .filter(|id| {
                self.collection
                    .declaration(*id)
                    .is_some_and(|declaration| declaration.kind() == DeclarationKind::Const)
            })
            .collect::<Vec<_>>();
        if self.evaluate_constant_declarations {
            self.check_constant_declarations(&constants)?;
        }
        for declaration in declarations {
            if self.evaluate_constant_declarations
                && self
                    .collection
                    .declaration(declaration)
                    .is_some_and(|declaration| declaration.kind() == DeclarationKind::Const)
            {
                continue;
            }
            self.check_declaration(declaration)?;
        }
        Ok(())
    }

    pub(super) fn check_declaration(&mut self, declaration: DefId) -> Result<(), BodyFlow> {
        self.ensure_analysis_active()?;
        let signature = match self.signatures.source_declaration(declaration) {
            FactState::Known(signature) => signature.clone(),
            FactState::Poisoned(poison) => {
                self.bodies
                    .insert(declaration, SemanticFact::Poisoned(poison.clone()))?;
                return Ok(());
            }
            FactState::Unperformed | FactState::NotApplicable => return Ok(()),
        };
        let previous_owner = self.current_owner.replace(declaration);
        let unavailable = self.recorder.unavailable_count();
        let result = (|| {
            match signature {
                DeclarationSignature::Callable(callable) => {
                    let callable = self
                        .signatures
                        .callable(callable)
                        .cloned()
                        .ok_or(BodyTypingError::UnknownCallable(callable))?;
                    if callable.body().is_some() {
                        self.check_callable_body(declaration, &callable)?;
                    }
                }
                DeclarationSignature::Const { ty } => self.check_const_body(declaration, ty)?,
                _ => {}
            }
            Ok(())
        })();
        self.current_owner = previous_owner;
        if self.recorder.unavailable_count() != unavailable {
            self.rejected_coverage.withhold_body(declaration);
        }
        #[cfg(test)]
        if let Some(fact) = self.bodies.recorded_state(&declaration) {
            execution::tests::sweep_support::published_body(declaration, fact);
        }
        result
    }

    pub(super) fn check_callable_body(
        &mut self,
        declaration: DefId,
        signature: &CallableSignature,
    ) -> Result<(), BodyFlow> {
        let (module, source, hir, item) = self.source_item(declaration)?;
        let ItemKind::Function(function) = item.kind() else {
            return Err(BodyFlow::Internal(BodyTypingError::WrongDeclarationKind(
                declaration,
            )));
        };
        let body = hir.body(function.body);
        let result_expected = function.return_type.map_or_else(
            || ExpectedType::caller(signature.result()),
            |type_ref| ExpectedType::written(signature.result(), source.file(), type_ref),
        );
        let result_expected =
            match self.normalize_expected_at(source, function.name.origin(), result_expected)? {
                Outcome::Known(ty) => ty,
                Outcome::Poisoned(poison) => {
                    return self.record_body_outcome(
                        declaration,
                        signature.result(),
                        Outcome::Poisoned(poison),
                    );
                }
                Outcome::Unperformed | Outcome::Deferred | Outcome::NotApplicable => return Ok(()),
            };
        let result_ty = result_expected.ty();
        let previous_return = self.current_return.replace(result_expected.clone());
        let region = ControlRegionKey::Callable(declaration);
        self.enter_control_region(region.clone())?;
        let outcome = (|| {
            let mut setup = UnitOutcomeFold::Known;
            for parameter in signature.parameters() {
                if let Some(pattern) = parameter.source_pattern() {
                    let origin = body
                        .pattern(pattern)
                        .ok_or(BodyTypingError::WrongBodyPattern(pattern))?
                        .origin();
                    let parameter_ty =
                        match self.normalize_type_at(source, origin, parameter.ty())? {
                            Outcome::Known(ty) => ty,
                            Outcome::Poisoned(poison) => {
                                setup = setup.include(Outcome::Poisoned(poison))?;
                                continue;
                            }
                            Outcome::Unperformed | Outcome::Deferred | Outcome::NotApplicable => {
                                setup = setup.include(Outcome::Unperformed)?;
                                continue;
                            }
                        };
                    let bound = self.bind_pattern(
                        module,
                        source,
                        body,
                        pattern,
                        parameter_ty,
                        PatternBindingMode::Parameter,
                    )?;
                    setup = setup.include(bound)?;
                }
            }
            match setup.finish() {
                Outcome::Known(()) => match function.body_form {
                    FunctionBody::Absent => Ok(Outcome::NotApplicable),
                    FunctionBody::Expression(expression) | FunctionBody::Block(expression) => {
                        let value = self.type_expression_in_current_region(
                            module,
                            source,
                            hir,
                            body,
                            expression,
                            TypeRequest::definitive(Some(result_expected)),
                        )?;
                        self.check_return_value(source, expression, result_ty, value)
                    }
                },
                Outcome::Poisoned(poison) => Ok(Outcome::Poisoned(poison)),
                Outcome::Unperformed => Ok(Outcome::Unperformed),
                Outcome::Deferred | Outcome::NotApplicable => Ok(Outcome::Unperformed),
            }
        })();
        let cleanup = self.leave_control_region(&region);
        self.current_return = previous_return;
        if let Err(error) = cleanup {
            self.retain_cleanup_failure(error.clone());
            if outcome.is_ok() {
                return Err(error.into());
            }
        }
        let outcome = outcome?;
        self.ensure_analysis_active()?;
        self.record_body_outcome(declaration, result_ty, outcome)
    }

    pub(super) fn check_const_body(
        &mut self,
        declaration: DefId,
        expected: TypeId,
    ) -> Result<(), BodyFlow> {
        let (module, source, hir, item) = self.source_item(declaration)?;
        let ItemKind::Const(value) = item.kind() else {
            return Err(BodyFlow::Internal(BodyTypingError::WrongDeclarationKind(
                declaration,
            )));
        };
        let body = hir.body(value.body);
        if self.evaluate_constant_declarations {
            self.require_constant_declarations(source, item.origin());
            let mut name_poison = eglur_diagnostics::PoisonAccumulator::new();
            for (expression, _) in body.expressions() {
                if let FactState::Poisoned(poison) = self
                    .names
                    .fact(ResolutionKey::Expression { module, expression })
                {
                    #[cfg(test)]
                    super::constants::poison_tests::observe(
                        super::constants::poison_tests::Step::Input(0),
                    );
                    name_poison.include(poison)?;
                }
            }
            if let Some(poison) = name_poison.finish() {
                #[cfg(test)]
                super::constants::poison_tests::observe(
                    super::constants::poison_tests::Step::Complete(0),
                );
                return self.record_body_outcome(declaration, expected, Outcome::Poisoned(poison));
            }
            if !self.is_const_evaluable_expression(module, body, value.value)? {
                if !self.has_nonconstant_call(module, body) {
                    let outcome = self.unsupported(
                        TypecheckCapability::EvaluateScalarConstExpressions,
                        source,
                        item.origin(),
                    );
                    return self.record_body_outcome(declaration, expected, outcome);
                }
                return self.reject_constant(
                    declaration,
                    body.expression(value.value)
                        .ok_or(BodyTypingError::WrongBodyExpression(value.value))?
                        .origin(),
                    "CONST_INIT_NOT_CONST",
                    "const initializer is not const-evaluable",
                );
            }
        }
        let original_expected = ExpectedType::written(expected, source.file(), value.ty);
        let expected = match self.normalize_expected_at(source, item.origin(), original_expected)? {
            Outcome::Known(ty) => ty,
            Outcome::Poisoned(poison) => {
                return self.record_body_outcome(declaration, expected, Outcome::Poisoned(poison));
            }
            Outcome::Unperformed | Outcome::Deferred | Outcome::NotApplicable => return Ok(()),
        };
        let outcome = self.type_expression(
            module,
            source,
            hir,
            body,
            value.value,
            TypeRequest::definitive(Some(expected.clone())),
        )?;
        let outcome = match outcome {
            Outcome::Known(actual)
                if self
                    .signatures
                    .types()
                    .is_assignable(actual.ty, expected.ty())? =>
            {
                Outcome::Known(actual)
            }
            Outcome::Known(actual) => {
                let origin = body
                    .expression(value.value)
                    .ok_or(BodyTypingError::WrongBodyExpression(value.value))?
                    .origin();
                Outcome::Poisoned(self.assignment_mismatch(
                    source,
                    origin,
                    expected.ty(),
                    actual.ty,
                )?)
            }
            other => other,
        };
        let outcome = if self.evaluate_constant_declarations
            && matches!(&outcome, Outcome::Known(value) if value.integer_constant.is_none())
        {
            self.unsupported(
                TypecheckCapability::EvaluateScalarConstExpressions,
                source,
                item.origin(),
            )
        } else {
            outcome
        };
        self.record_body_outcome(declaration, expected.ty(), outcome)
    }

    pub(super) fn record_body_outcome(
        &mut self,
        declaration: DefId,
        result: TypeId,
        outcome: Outcome<TypedExpression>,
    ) -> Result<(), BodyFlow> {
        let fact = match outcome {
            Outcome::Known(_) => Some(SemanticFact::Known(BodyTypeSummary { result })),
            Outcome::Poisoned(poison) => Some(SemanticFact::Poisoned(poison)),
            Outcome::NotApplicable => Some(SemanticFact::NotApplicable),
            Outcome::Deferred | Outcome::Unperformed => None,
        };
        if let Some(fact) = fact {
            self.bodies.insert(declaration, fact)?;
        }
        Ok(())
    }

    pub(super) fn source_item(
        &self,
        declaration: DefId,
    ) -> Result<
        (
            ModuleId,
            &'input SemanticSource<'hir>,
            &'hir HirFile,
            &'hir eglur_hir::Item,
        ),
        BodyTypingError,
    > {
        let declaration_record = self
            .collection
            .declaration(declaration)
            .ok_or(BodyTypingError::UnknownDeclaration(declaration))?;
        let module = self
            .collection
            .module(declaration.module())
            .ok_or(BodyTypingError::UnknownModule(declaration.module()))?;
        let source = self
            .sources
            .get(&module.file())
            .copied()
            .ok_or(BodyTypingError::MissingSource(module.file()))?;
        let hir = source
            .hir()
            .ok_or(BodyTypingError::MissingHir(module.file()))?;
        let item = declaration_record
            .item()
            .ok_or(BodyTypingError::MissingDeclarationItem(declaration))?;
        Ok((module.id(), source, hir, hir.item(item)))
    }
}
