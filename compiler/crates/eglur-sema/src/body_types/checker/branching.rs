use super::*;

#[cfg(test)]
mod poison_tests;

pub(super) enum BranchPlan {
    Complete(Outcome<TypedExpression>),
    Expression {
        expression: ExprId,
        request: TypeRequest,
        continuation: BranchContinuation,
    },
    IfBranch {
        region: ControlRegionKey,
        path: NormalContinuation,
        expression: ExprId,
        request: TypeRequest,
        frame: Box<IfFrame>,
    },
    MatchArm {
        region: ControlRegionKey,
        path: NormalContinuation,
        frame: Box<MatchFrame>,
    },
    MatchBody {
        region: ControlRegionKey,
        path: NormalContinuation,
        expression: ExprId,
        request: TypeRequest,
        frame: Box<MatchFrame>,
    },
    CompleteMatchArm(Box<MatchFrame>),
}

pub(super) enum BranchContinuation {
    EmptyMatch(OriginId),
    IfCondition {
        owner: ExpressionKey,
        origin: OriginId,
        expression: eglur_hir::IfExpr,
        request: TypeRequest,
    },
    IfBranch(Box<IfFrame>),
    MatchScrutinee {
        owner: ExpressionKey,
        expression: eglur_hir::MatchExpr,
        request: TypeRequest,
    },
    MatchGuard {
        frame: Box<MatchFrame>,
        bool_ty: TypeId,
    },
    MatchValue {
        frame: Box<MatchFrame>,
        guard_continuation: NormalContinuation,
    },
}

pub(super) struct IfFrame {
    owner: ExpressionKey,
    origin: OriginId,
    expression: eglur_hir::IfExpr,
    request: TypeRequest,
    next_branch: usize,
    condition_continuation: NormalContinuation,
    common: Option<TypeId>,
    any_normal_branch: bool,
    poison: Option<Poison>,
    unperformed: bool,
    deferred: bool,
}

impl IfFrame {
    fn next_branch(&mut self) -> Option<(IfBranch, ExprId)> {
        let branches = [
            (IfBranch::Then, Some(self.expression.then_branch)),
            (IfBranch::Else, self.expression.else_branch),
            (IfBranch::Unknown, self.expression.unknown_branch),
        ];
        while let Some(&(slot, expression)) = branches.get(self.next_branch) {
            self.next_branch += 1;
            if let Some(expression) = expression {
                return Some((slot, expression));
            }
        }
        None
    }
}

pub(super) struct MatchFrame {
    owner: ExpressionKey,
    expression: eglur_hir::MatchExpr,
    request: TypeRequest,
    next_arm: usize,
    scrutinee_type: Option<TypeId>,
    scrutinee_continuation: NormalContinuation,
    common: Option<TypeId>,
    any_normal_arm: bool,
    patterns: Vec<PatternCheck>,
    poison: PendingPoison,
    unperformed: bool,
    deferred: bool,
}

impl MatchFrame {
    fn region(&self) -> ControlRegionKey {
        ControlRegionKey::MatchArm {
            owner: self.owner,
            arm: self.next_arm,
        }
    }

    fn body_plan(self: Box<Self>, path: NormalContinuation) -> BranchPlan {
        BranchPlan::MatchBody {
            region: self.region(),
            path,
            expression: self.expression.arms[self.next_arm].value,
            request: self
                .request
                .definitive_child(self.common.or(self.request.expected_type())),
            frame: self,
        }
    }
}

impl<'input, 'hir, 'support> BodyChecker<'input, 'hir, 'support> {
    pub(super) fn prepare_if(
        &mut self,
        source: &SemanticSource<'_>,
        owner: ExpressionKey,
        origin: OriginId,
        expression: eglur_hir::IfExpr,
        request: TypeRequest,
    ) -> BranchPlan {
        if !self.recorder.require(
            CompileCapability::Typecheck(TypecheckCapability::CheckBranching),
            SemanticLocation::origin(source.file(), origin),
        ) {
            return BranchPlan::Complete(Outcome::Unperformed);
        }
        BranchPlan::Expression {
            expression: expression.condition,
            request: TypeRequest::definitive(None),
            continuation: BranchContinuation::IfCondition {
                owner,
                origin,
                expression,
                request,
            },
        }
    }

    pub(super) fn prepare_match(
        &mut self,
        source: &SemanticSource<'_>,
        owner: ExpressionKey,
        origin: OriginId,
        expression: eglur_hir::MatchExpr,
        request: TypeRequest,
    ) -> BranchPlan {
        let capability = if expression.arms.is_empty() {
            TypecheckCapability::AnalyzeInhabitance
        } else {
            TypecheckCapability::CheckBranching
        };
        if !self.recorder.require(
            CompileCapability::Typecheck(capability),
            SemanticLocation::origin(source.file(), origin),
        ) {
            return BranchPlan::Complete(Outcome::Unperformed);
        }
        BranchPlan::Expression {
            expression: expression.scrutinee,
            request: TypeRequest::definitive(None),
            continuation: if expression.arms.is_empty() {
                BranchContinuation::EmptyMatch(origin)
            } else {
                BranchContinuation::MatchScrutinee {
                    owner,
                    expression,
                    request,
                }
            },
        }
    }

    pub(super) fn resume_branch(
        &mut self,
        context: BodyContext<'_, '_>,
        continuation: BranchContinuation,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        match continuation {
            BranchContinuation::EmptyMatch(origin) => {
                self.finish_empty_match(context.source, origin, outcome)
            }
            BranchContinuation::IfCondition {
                owner,
                origin,
                expression,
                request,
            } => self.resume_if_condition(
                context.source,
                owner,
                origin,
                expression,
                request,
                outcome,
            ),
            BranchContinuation::IfBranch(frame) => {
                self.resume_if_branch(context.source, frame, outcome)
            }
            BranchContinuation::MatchScrutinee {
                owner,
                expression,
                request,
            } => self.resume_match_scrutinee(context.source, owner, expression, request, outcome),
            BranchContinuation::MatchGuard { frame, bool_ty } => {
                self.resume_match_guard(context.source, frame, bool_ty, outcome)
            }
            BranchContinuation::MatchValue {
                frame,
                guard_continuation,
            } => self.resume_match_value(context.source, frame, guard_continuation, outcome),
        }
    }

    fn finish_empty_match(
        &mut self,
        source: &SemanticSource<'_>,
        origin: OriginId,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        let Outcome::Known(scrutinee) = outcome else {
            return Ok(BranchPlan::Complete(outcome));
        };
        let outcome = match self.inhabitance.classify(self.signatures, scrutinee.ty)? {
            Inhabitance::Uninhabited => {
                Outcome::Known(self.value_expression(self.signatures.types().never()))
            }
            Inhabitance::Unknown | Inhabitance::Inhabited => Outcome::Poisoned(self.type_error(
                source,
                origin,
                "MATCH_NON_EXHAUSTIVE",
                "match is non-exhaustive for a possibly inhabited scrutinee",
                BTreeMap::new(),
            )?),
        };
        Ok(BranchPlan::Complete(outcome))
    }

    fn resume_if_condition(
        &mut self,
        source: &SemanticSource<'_>,
        owner: ExpressionKey,
        origin: OriginId,
        expression: eglur_hir::IfExpr,
        request: TypeRequest,
        condition: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        let mut frame = Box::new(IfFrame {
            owner,
            origin,
            any_normal_branch: expression.else_branch.is_none(),
            expression,
            request,
            next_branch: 0,
            condition_continuation: NormalContinuation::Absent,
            common: None,
            poison: None,
            unperformed: false,
            deferred: false,
        });
        match condition {
            Outcome::Known(condition) => {
                let bool_ty = self.primitive(PrimitiveType::Bool)?;
                if condition.ty != bool_ty {
                    let found = self.type_error(
                        source,
                        self.expression_origin(source, frame.expression.condition)?,
                        "IF_CONDITION_NOT_BOOL",
                        "if condition must have type Bool or Tri",
                        BTreeMap::new(),
                    )?;
                    self.merge_poison(&mut frame.poison, &found)?;
                } else if frame.expression.unknown_branch.is_some() {
                    let found = self.type_error(
                        source,
                        origin,
                        "IF_BOOL_UNKNOWN_BRANCH_FOR_BOOL",
                        "if Bool condition has a forbidden unknown branch",
                        BTreeMap::new(),
                    )?;
                    self.merge_poison(&mut frame.poison, &found)?;
                }
                frame.condition_continuation = condition.normal_continuation;
            }
            Outcome::Poisoned(found) => self.merge_poison(&mut frame.poison, &found)?,
            Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
            Outcome::Deferred => frame.deferred = true,
        }
        if frame.expression.else_branch.is_none() && frame.request.value_required {
            let found = self.type_error(
                source,
                origin,
                "IF_MISSING_ELSE_VALUE",
                "if without else cannot be used as a value",
                BTreeMap::new(),
            )?;
            self.merge_poison(&mut frame.poison, &found)?;
        }
        self.advance_if(frame)
    }

    fn advance_if(&mut self, mut frame: Box<IfFrame>) -> Result<BranchPlan, BodyFlow> {
        if let Some((branch, expression)) = frame.next_branch() {
            return Ok(BranchPlan::IfBranch {
                region: ControlRegionKey::IfBranch {
                    owner: frame.owner,
                    branch,
                },
                path: frame.condition_continuation,
                expression,
                request: frame
                    .request
                    .definitive_child(frame.common.or(frame.request.expected_type())),
                frame,
            });
        }
        if let Some(poison) = frame.poison {
            return Ok(BranchPlan::Complete(Outcome::Poisoned(poison)));
        }
        if frame.unperformed {
            return Ok(BranchPlan::Complete(Outcome::Unperformed));
        }
        if frame.deferred {
            return Ok(BranchPlan::Complete(Outcome::Deferred));
        }
        let ty = if frame.expression.else_branch.is_none() {
            self.primitive(PrimitiveType::Unit)?
        } else {
            frame
                .common
                .unwrap_or_else(|| self.signatures.types().never())
        };
        let branch_continuation = if frame.any_normal_branch {
            NormalContinuation::Present
        } else {
            NormalContinuation::Absent
        };
        Ok(BranchPlan::Complete(Outcome::Known(
            self.value_expression_with_continuation(
                ty,
                frame.condition_continuation.strict(branch_continuation),
            ),
        )))
    }

    fn resume_if_branch(
        &mut self,
        source: &SemanticSource<'_>,
        mut frame: Box<IfFrame>,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        match outcome {
            Outcome::Known(branch) if !branch.normal_continuation.is_present() => {}
            Outcome::Known(branch) => {
                frame.any_normal_branch = true;
                if frame.expression.else_branch.is_some() {
                    if let Some(left) = frame.common {
                        if let Some(joined) = self.branch_common_type(left, branch.ty)? {
                            frame.common = Some(joined);
                        } else {
                            let found = self.type_error(
                                source,
                                frame.origin,
                                "IF_BRANCH_TYPE_MISMATCH",
                                "if branch result types mismatch",
                                BTreeMap::from([
                                    (
                                        Box::<str>::from("left_type"),
                                        DiagnosticDataValue::from(self.format_type(left)?),
                                    ),
                                    (
                                        Box::<str>::from("right_type"),
                                        DiagnosticDataValue::from(self.format_type(branch.ty)?),
                                    ),
                                ]),
                            )?;
                            self.merge_poison(&mut frame.poison, &found)?;
                        }
                    } else {
                        frame.common = Some(branch.ty);
                    }
                }
            }
            Outcome::Poisoned(found) => self.merge_poison(&mut frame.poison, &found)?,
            Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
            Outcome::Deferred => frame.deferred = true,
        }
        self.advance_if(frame)
    }

    fn resume_match_scrutinee(
        &mut self,
        source: &SemanticSource<'_>,
        owner: ExpressionKey,
        expression: eglur_hir::MatchExpr,
        request: TypeRequest,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        let mut frame = Box::new(MatchFrame {
            owner,
            patterns: Vec::with_capacity(expression.arms.len()),
            expression,
            request,
            next_arm: 0,
            scrutinee_type: None,
            scrutinee_continuation: NormalContinuation::Absent,
            common: None,
            any_normal_arm: false,
            poison: PendingPoison::default(),
            unperformed: false,
            deferred: false,
        });
        match outcome {
            Outcome::Known(value) => {
                frame.scrutinee_type = Some(value.ty);
                frame.scrutinee_continuation = value.normal_continuation;
            }
            Outcome::Poisoned(found) => frame.poison.include(&found)?,
            Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
            Outcome::Deferred => frame.deferred = true,
        }
        self.advance_match(source, frame)
    }

    pub(super) fn advance_match(
        &mut self,
        source: &SemanticSource<'_>,
        mut frame: Box<MatchFrame>,
    ) -> Result<BranchPlan, BodyFlow> {
        if frame.next_arm < frame.expression.arms.len() {
            return Ok(BranchPlan::MatchArm {
                region: frame.region(),
                path: frame.scrutinee_continuation,
                frame,
            });
        }
        if !frame.poison.has_poison()
            && !frame.unperformed
            && !frame.deferred
            && let Some(expected) = frame.scrutinee_type
        {
            match self.check_match_coverage(
                source,
                expected,
                &frame.expression.arms,
                &frame.patterns,
            )? {
                Outcome::Known(arms) => {
                    if let Some(existing) = self.carrier_arms.get(&frame.owner) {
                        if Some(existing.as_ref()) != arms.as_deref() {
                            return Err(DuplicateFact.into());
                        }
                    } else if let Some(arms) = arms {
                        self.carrier_arms
                            .insert(frame.owner, arms.into_boxed_slice());
                    }
                }
                Outcome::Poisoned(found) => frame.poison.include(&found)?,
                Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
                Outcome::Deferred => frame.deferred = true,
            }
        }
        if let Some(poison) = frame.poison.take() {
            return Ok(BranchPlan::Complete(Outcome::Poisoned(poison)));
        }
        if frame.unperformed {
            return Ok(BranchPlan::Complete(Outcome::Unperformed));
        }
        if frame.deferred {
            return Ok(BranchPlan::Complete(Outcome::Deferred));
        }
        let ty = frame
            .common
            .unwrap_or_else(|| self.signatures.types().never());
        let arm_continuation = if frame.any_normal_arm {
            NormalContinuation::Present
        } else {
            NormalContinuation::Absent
        };
        Ok(BranchPlan::Complete(Outcome::Known(
            self.value_expression_with_continuation(
                ty,
                frame.scrutinee_continuation.strict(arm_continuation),
            ),
        )))
    }

    pub(super) fn prepare_match_arm(
        &mut self,
        context: BodyContext<'_, '_>,
        mut frame: Box<MatchFrame>,
    ) -> Result<BranchPlan, BodyFlow> {
        let BodyContext { module, body, .. } = context;
        let arm = &frame.expression.arms[frame.next_arm];
        let pattern = if let Some(expected) = frame.scrutinee_type {
            self.type_match_pattern(context, arm.pattern, expected)?
        } else {
            Outcome::Unperformed
        };
        match pattern {
            Outcome::Known(pattern) => {
                self.install_match_bindings(module, arm.pattern)?;
                frame.patterns.push(pattern);
            }
            Outcome::Poisoned(found) => {
                self.bind_pattern_poison(module, body, arm.pattern, found.clone())?;
                frame.poison.include(&found)?;
            }
            Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
            Outcome::Deferred => frame.deferred = true,
        }
        if let Some(guard) = &arm.guard {
            let bool_ty = self.primitive(PrimitiveType::Bool)?;
            return Ok(BranchPlan::Expression {
                expression: guard.condition,
                request: TypeRequest::local(Some(bool_ty)),
                continuation: BranchContinuation::MatchGuard { frame, bool_ty },
            });
        }
        Ok(frame.body_plan(NormalContinuation::Present))
    }

    fn resume_match_guard(
        &mut self,
        source: &SemanticSource<'_>,
        mut frame: Box<MatchFrame>,
        bool_ty: TypeId,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        let guard = frame.expression.arms[frame.next_arm]
            .guard
            .as_ref()
            .expect("guard continuation requires a guard");
        let condition = guard.condition;
        let guard_continuation = match outcome {
            Outcome::Known(value) => {
                if value.ty != bool_ty {
                    let found = self.type_error(
                        source,
                        self.expression_origin(source, condition)?,
                        "MATCH_GUARD_TYPE_MISMATCH",
                        "match guard must have type Bool",
                        BTreeMap::from([
                            (
                                Box::<str>::from("expected_type"),
                                DiagnosticDataValue::from("Bool"),
                            ),
                            (
                                Box::<str>::from("actual_type"),
                                DiagnosticDataValue::from(self.format_type(value.ty)?),
                            ),
                        ]),
                    )?;
                    frame.poison.include(&found)?;
                }
                value.normal_continuation
            }
            Outcome::Poisoned(found) => {
                frame.poison.include(&found)?;
                NormalContinuation::Absent
            }
            Outcome::Unperformed | Outcome::NotApplicable => {
                frame.unperformed = true;
                NormalContinuation::Absent
            }
            Outcome::Deferred => {
                frame.deferred = true;
                NormalContinuation::Absent
            }
        };
        if guard_continuation.is_present() {
            self.record_control_transfer(
                ControlTransferSource::Expression(ExpressionKey::new(
                    frame.owner.module(),
                    condition,
                )),
                ControlTransferKind::MatchGuardFalse,
                frame.region(),
            )?;
        }
        Ok(frame.body_plan(guard_continuation))
    }

    fn resume_match_value(
        &mut self,
        source: &SemanticSource<'_>,
        mut frame: Box<MatchFrame>,
        guard_continuation: NormalContinuation,
        outcome: Outcome<TypedExpression>,
    ) -> Result<BranchPlan, BodyFlow> {
        match outcome {
            Outcome::Known(value)
                if !guard_continuation.is_present() || !value.normal_continuation.is_present() => {}
            Outcome::Known(value) => {
                frame.any_normal_arm = true;
                if let Some(left) = frame.common {
                    if let Some(joined) = self.branch_common_type(left, value.ty)? {
                        frame.common = Some(joined);
                    } else {
                        let found = self.type_error(
                            source,
                            self.expression_origin(
                                source,
                                frame.expression.arms[frame.next_arm].value,
                            )?,
                            "MATCH_ARM_TYPE_MISMATCH",
                            "match arm result types mismatch",
                            BTreeMap::from([
                                (
                                    Box::<str>::from("left_type"),
                                    DiagnosticDataValue::from(self.format_type(left)?),
                                ),
                                (
                                    Box::<str>::from("right_type"),
                                    DiagnosticDataValue::from(self.format_type(value.ty)?),
                                ),
                            ]),
                        )?;
                        frame.poison.include(&found)?;
                    }
                } else {
                    frame.common = Some(value.ty);
                }
            }
            Outcome::Poisoned(found) => frame.poison.include(&found)?,
            Outcome::Unperformed | Outcome::NotApplicable => frame.unperformed = true,
            Outcome::Deferred => frame.deferred = true,
        }
        frame.next_arm += 1;
        Ok(BranchPlan::CompleteMatchArm(frame))
    }
}
