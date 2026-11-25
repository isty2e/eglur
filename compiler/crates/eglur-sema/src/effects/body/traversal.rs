use std::rc::Rc;

use super::*;
use eglur_hir::MatchArm;

enum Branch<'hir> {
    Expression(ExprId),
    MatchArm(&'hir MatchArm, Rc<BTreeSet<MutableRoot>>),
    Unchanged,
}

enum Step<'hir> {
    Expression(ExprId),
    Statement(StmtId),
    Call(ExprId, OriginId),
    Bind(PatternId, Option<ExprId>),
    Assign(OriginId, ExprId, ExprId),
    Escape(ExprId),
    Match(ExprId, &'hir [MatchArm]),
    Fork(Vec<Branch<'hir>>),
    StartBranch(Branch<'hir>),
    FinishBranch,
    Join,
}

impl BodyCollector<'_> {
    pub(super) fn visit_expression(
        &mut self,
        expression: ExprId,
        state: &mut FlowState,
    ) -> Result<(), EffectBodyError> {
        let mut pending = vec![Step::Expression(expression)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Expression(id) => self.schedule_expression(id, &mut pending)?,
                Step::Statement(id) => self.schedule_statement(id, &mut pending)?,
                Step::Call(id, origin) => self.record_call(id, origin, state)?,
                Step::Bind(pattern, initializer) => {
                    let roots = initializer.map_or_else(
                        || Ok(BTreeSet::new()),
                        |id| self.expression_roots(id, state),
                    )?;
                    self.bind_aliases(pattern, &roots, state)?;
                }
                Step::Assign(origin, target, value) => {
                    self.finish_assignment(origin, target, value, state)?;
                }
                Step::Escape(id) => state.mark_escaping(self.expression_roots(id, state)?),
                Step::Match(scrutinee, arms) => {
                    let roots = self.expression_roots(scrutinee, state)?;
                    if !arms.is_empty() {
                        let roots = Rc::new(roots);
                        pending.push(Step::Fork(
                            arms.iter()
                                .map(|arm| Branch::MatchArm(arm, Rc::clone(&roots)))
                                .collect(),
                        ));
                    }
                }
                Step::Fork(branches) => {
                    state.begin_fork(branches.len());
                    pending.push(Step::Join);
                    for branch in branches.into_iter().rev() {
                        pending.push(Step::FinishBranch);
                        pending.push(Step::StartBranch(branch));
                    }
                }
                Step::StartBranch(branch) => match branch {
                    Branch::Expression(id) => pending.push(Step::Expression(id)),
                    Branch::MatchArm(arm, roots) => {
                        self.bind_aliases(arm.pattern, &roots, state)?;
                        pending.push(Step::Expression(arm.value));
                        if let Some(guard) = &arm.guard {
                            pending.push(Step::Expression(guard.condition));
                        }
                    }
                    Branch::Unchanged => {}
                },
                Step::FinishBranch => state.finish_arm(),
                Step::Join => state.finish_fork(),
            }
        }
        Ok(())
    }

    fn schedule_expression<'hir>(
        &mut self,
        id: ExprId,
        pending: &mut Vec<Step<'hir>>,
    ) -> Result<(), EffectBodyError>
    where
        Self: 'hir,
    {
        let expression = self
            .body
            .expression(id)
            .ok_or(EffectBodyError::WrongBodyExpression(id))?;
        match expression.kind() {
            ExprKind::Block { statements } => {
                pending.extend(statements.iter().rev().copied().map(Step::Statement))
            }
            ExprKind::Binary { left, right, .. } => {
                pending.push(Step::Expression(*right));
                pending.push(Step::Expression(*left));
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::Reference { value: operand, .. }
            | ExprKind::Try(operand)
            | ExprKind::Group(operand)
            | ExprKind::Unsafe(operand)
            | ExprKind::Result(operand)
            | ExprKind::Checked(operand) => pending.push(Step::Expression(*operand)),
            ExprKind::Call(call) => {
                pending.push(Step::Call(id, expression.origin()));
                pending.extend(
                    call.arguments
                        .iter()
                        .rev()
                        .map(|arg| Step::Expression(arg.value)),
                );
                pending.push(Step::Expression(call.callee));
            }
            ExprKind::CatalogCall(call) => {
                pending.push(Step::Call(id, expression.origin()));
                pending.extend(
                    call.arguments
                        .iter()
                        .rev()
                        .map(|arg| Step::Expression(arg.value)),
                );
            }
            ExprKind::DTypeCall(call) => {
                pending.push(Step::Call(id, expression.origin()));
                pending.extend(
                    call.arguments
                        .iter()
                        .rev()
                        .map(|arg| Step::Expression(arg.value)),
                );
                pending.push(Step::Expression(call.callee));
            }
            ExprKind::Member { receiver, .. } => {
                if !matches!(
                    self.names.fact(ResolutionKey::Expression {
                        module: self.module,
                        expression: id,
                    }),
                    FactState::Known(ResolvedName::TypeMemberOwner(_))
                ) {
                    pending.push(Step::Expression(*receiver));
                }
            }
            ExprKind::TupleProjection { receiver, .. } => {
                pending.push(Step::Expression(*receiver));
            }
            ExprKind::ReduceByCandidate(candidate) => {
                match self
                    .body_types
                    .reduce_by_commitment(ExpressionKey::new(self.module, id))
                {
                    FactState::Known(ReduceByCommitment::Ordinary) => {
                        let ordinary = candidate
                            .ordinary
                            .as_ref()
                            .expect("ordinary commitment retains an ordinary candidate");
                        pending.push(Step::Call(id, expression.origin()));
                        pending.extend(
                            ordinary
                                .call
                                .arguments
                                .iter()
                                .rev()
                                .map(|arg| Step::Expression(arg.value)),
                        );
                        pending.push(Step::Expression(ordinary.call.callee));
                    }
                    FactState::Known(ReduceByCommitment::TensorSingle) => {
                        let single = candidate
                            .single_reducer
                            .as_ref()
                            .expect("single commitment retains a reducer");
                        let value = match single.value {
                            ReduceBySingleValue::Expression(value)
                            | ReduceBySingleValue::PhasePair { reducer: value, .. } => value,
                        };
                        pending.push(Step::Expression(value));
                        pending.push(Step::Expression(candidate.receiver));
                    }
                    FactState::Known(ReduceByCommitment::TensorPhases) => {
                        let phases = candidate
                            .ordered_phases
                            .as_ref()
                            .expect("phase commitment retains ordered phases");
                        pending.extend(
                            phases
                                .phases
                                .iter()
                                .rev()
                                .map(|phase| Step::Expression(phase.reducer)),
                        );
                        pending.push(Step::Expression(candidate.receiver));
                    }
                    FactState::Poisoned(poison) => self.block_on_poison(poison)?,
                    FactState::Unperformed | FactState::NotApplicable => {
                        self.block_on_unperformed()
                    }
                }
            }
            ExprKind::Tuple(values) | ExprKind::Array(values) => {
                pending.extend(values.iter().rev().copied().map(Step::Expression))
            }
            ExprKind::Map(entries) => {
                for entry in entries.iter().rev() {
                    pending.push(Step::Expression(entry.value));
                    pending.push(Step::Expression(entry.key));
                }
            }
            ExprKind::StructLiteral { fields, .. } => pending.extend(
                fields
                    .iter()
                    .rev()
                    .map(|field| Step::Expression(field.value)),
            ),
            ExprKind::VariantConstructor { payload, .. } => match payload {
                ConstructorPayload::Absent => {}
                ConstructorPayload::Tuple(values) => {
                    pending.extend(values.iter().rev().copied().map(Step::Expression))
                }
                ConstructorPayload::Named(fields) => pending.extend(
                    fields
                        .iter()
                        .rev()
                        .map(|field| Step::Expression(field.value)),
                ),
            },
            ExprKind::If(value) => {
                let mut branches = vec![Branch::Expression(value.then_branch)];
                branches.extend(
                    [value.else_branch, value.unknown_branch]
                        .into_iter()
                        .flatten()
                        .map(Branch::Expression),
                );
                if value.else_branch.is_none() {
                    branches.push(Branch::Unchanged);
                }
                pending.push(Step::Fork(branches));
                pending.push(Step::Expression(value.condition));
            }
            ExprKind::Match(value) => {
                pending.push(Step::Match(value.scrutinee, &value.arms));
                pending.push(Step::Expression(value.scrutinee));
            }
            ExprKind::Own(_)
            | ExprKind::Move(_)
            | ExprKind::DropOwn { .. }
            | ExprKind::Literal(_)
            | ExprKind::Name(_)
            | ExprKind::Hole
            | ExprKind::Unit
            | ExprKind::Lambda(_)
            | ExprKind::TensorSchema(_)
            | ExprKind::Invalid => {}
        }
        Ok(())
    }

    fn schedule_statement(
        &self,
        id: StmtId,
        pending: &mut Vec<Step<'_>>,
    ) -> Result<(), EffectBodyError> {
        let statement = self
            .body
            .statement(id)
            .ok_or(EffectBodyError::WrongBodyStatement(id))?;
        match statement.kind() {
            StmtKind::Let {
                pattern,
                initializer,
                ..
            } => {
                pending.push(Step::Bind(*pattern, *initializer));
                if let Some(id) = initializer {
                    pending.push(Step::Expression(*id));
                }
            }
            StmtKind::Assign { target, value } => {
                pending.push(Step::Assign(statement.origin(), *target, *value));
                pending.push(Step::Expression(*value));
                pending.push(Step::Expression(*target));
            }
            StmtKind::While { condition, body } => {
                pending.push(Step::Fork(vec![
                    Branch::Unchanged,
                    Branch::Expression(*body),
                ]));
                pending.push(Step::Expression(*condition));
            }
            StmtKind::For { iterable, body, .. } => {
                pending.push(Step::Fork(vec![
                    Branch::Unchanged,
                    Branch::Expression(*body),
                ]));
                pending.push(Step::Expression(*iterable));
            }
            StmtKind::Return(value) => {
                if let Some(id) = value {
                    pending.push(Step::Escape(*id));
                    pending.push(Step::Expression(*id));
                }
            }
            StmtKind::Yield(id) => {
                pending.push(Step::Escape(*id));
                pending.push(Step::Expression(*id));
            }
            StmtKind::Expression(id) => pending.push(Step::Expression(*id)),
            StmtKind::Break | StmtKind::Continue | StmtKind::Invalid => {}
        }
        Ok(())
    }

    fn finish_assignment(
        &mut self,
        origin: OriginId,
        target: ExprId,
        value: ExprId,
        state: &mut FlowState,
    ) -> Result<(), EffectBodyError> {
        let target_expression = self
            .body
            .expression(target)
            .ok_or(EffectBodyError::WrongBodyExpression(target))?;
        if !matches!(target_expression.kind(), ExprKind::Hole) {
            let target_roots = self.assignment_roots(target, state)?;
            self.operations
                .push(EffectOperation::Mutation(EffectMutation {
                    origin,
                    path: MutablePath::new(target_roots.iter().cloned()),
                }));
            if target_roots.iter().any(|root| {
                matches!(
                    root,
                    MutableRoot::Boundary(_) | MutableRoot::Nonlocal | MutableRoot::Unknown
                )
            }) {
                state.mark_escaping(self.expression_roots(value, state)?);
            }
            self.update_reference_alias(target, value, state)?;
        }
        Ok(())
    }
}
