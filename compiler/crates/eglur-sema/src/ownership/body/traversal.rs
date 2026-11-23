use super::*;

struct BlockFrame<'hir> {
    remaining: std::slice::Iter<'hir, StmtId>,
    region: ControlRegionKey,
    origin: OriginId,
    provenance: ValueProvenance,
}

enum ValueSequenceKind {
    Call {
        in_flight: Vec<(OriginId, BTreeSet<LocalBindingId>)>,
    },
    Tuple {
        index: usize,
        provenance: ValueProvenance,
    },
    Array,
    Discard,
    Constructor,
}

struct ValueSequence {
    remaining: std::vec::IntoIter<ExprId>,
    kind: ValueSequenceKind,
}

impl ValueSequence {
    fn new(values: impl IntoIterator<Item = ExprId>, kind: ValueSequenceKind) -> Self {
        Self {
            remaining: values.into_iter().collect::<Vec<_>>().into_iter(),
            kind,
        }
    }

    fn visits_after_termination(&self) -> bool {
        matches!(
            self.kind,
            ValueSequenceKind::Discard | ValueSequenceKind::Constructor
        )
    }
}

struct IfFrame {
    owner: ExpressionKey,
    remaining: std::array::IntoIter<(IfBranch, Option<ExprId>), 3>,
    branches: Alternatives,
    provenance: ValueProvenance,
}

struct MatchFrame<'hir> {
    owner: ExpressionKey,
    remaining: std::iter::Enumerate<std::slice::Iter<'hir, eglur_hir::MatchArm>>,
    branches: Alternatives,
    subject: ValueProvenance,
    result: ValueProvenance,
}

impl<'hir> MatchFrame<'hir> {
    fn schedule_value(
        self,
        region: ControlRegionKey,
        arm: &eglur_hir::MatchArm,
        pending: &mut Vec<Step<'hir>>,
    ) {
        pending.push(Step::MatchValue(self, region, arm.origin));
        pending.push(Step::Expression(arm.value, None));
    }
}

enum TransferKind {
    Return,
    Yield,
}

#[derive(Clone, Copy)]
enum ProjectionReceiver {
    LocalAlias(LocalBindingId),
    Value,
}

enum Step<'hir> {
    Expression(ExprId, Option<ControlRegionKey>),
    Statement(StmtId),
    FinishExpression(ExpressionKey, OriginId),
    Reference(ExprId),
    Projection {
        expression: ExpressionKey,
        origin: OriginId,
        index: &'hir str,
        receiver: ProjectionReceiver,
    },
    RecoverMove(ExprId),
    Recoverable(ControlRegionKey),
    IfCondition(ExpressionKey, &'hir eglur_hir::IfExpr),
    IfNext(IfFrame),
    IfValue(IfFrame),
    MatchSubject(ExpressionKey, &'hir eglur_hir::MatchExpr),
    MatchNext(MatchFrame<'hir>),
    MatchGuard(
        MatchFrame<'hir>,
        ControlRegionKey,
        &'hir eglur_hir::MatchArm,
    ),
    MatchValue(MatchFrame<'hir>, ControlRegionKey, OriginId),
    BlockNext(BlockFrame<'hir>),
    BlockValue(BlockFrame<'hir>),
    ValuesNext(ValueSequence),
    Value(ValueSequence, ExprId),
    CallCallee(ValueSequence, ExprId),
    Let(PatternId, ExprId),
    AssignmentTarget(ExprId, ExprId),
    AssignmentValue(ExprId, bool),
    DiscardProvenance,
    Transfer(StatementKey, OriginId, TransferKind),
}

impl<'hir> BodyAnalyzer<'hir> {
    pub(super) fn visit_expression(
        &mut self,
        expression: ExprId,
        state: &mut FlowState,
        region_override: Option<ControlRegionKey>,
    ) -> Result<ExpressionFlow, OwnershipAnalysisError> {
        let region_depth = self.recoverable_exits.len();
        let result = self.run_steps(expression, state, region_override);
        // An analysis error must unwind region collectors as well as pending work.
        self.recoverable_exits.truncate(region_depth);
        result
    }

    fn run_steps(
        &mut self,
        expression: ExprId,
        state: &mut FlowState,
        region_override: Option<ControlRegionKey>,
    ) -> Result<ExpressionFlow, OwnershipAnalysisError> {
        let mut pending = vec![Step::Expression(expression, region_override)];
        let mut flow = ExpressionFlow::continuing();
        while let Some(step) = pending.pop() {
            match step {
                Step::Expression(id, region) => {
                    self.schedule_expression(id, region, state, &mut flow, &mut pending)?;
                }
                Step::Statement(id) => {
                    self.schedule_statement(id, &mut flow, &mut pending)?;
                }
                Step::FinishExpression(key, origin) => {
                    self.check_side_exits(key, origin, state);
                    flow.continuing = self.expression_continues(key, flow.continuing);
                }
                Step::Reference(value) => {
                    let mut roots = flow.provenance.roots();
                    roots.extend(self.expression_roots(value, state)?);
                    flow.provenance = ValueProvenance::direct(roots);
                }
                Step::Projection {
                    expression: key,
                    origin,
                    index,
                    receiver,
                } => {
                    let index = index.parse::<usize>().map_err(|_| {
                        OwnershipAnalysisError::InvalidTupleProjectionIndex {
                            expression: key,
                            index: index.into(),
                        }
                    })?;
                    flow.provenance = match receiver {
                        ProjectionReceiver::LocalAlias(binding) => state
                            .alias(binding)
                            .map(|provenance| provenance.projected(index))
                            .unwrap_or_default(),
                        ProjectionReceiver::Value => flow.provenance.projected(index),
                    };
                    self.use_roots(&flow.provenance.roots(), origin, state);
                }
                Step::RecoverMove(id) => self.recover_missing_move(id, state)?,
                Step::Recoverable(region) => {
                    let (recorded_region, exits) = self
                        .recoverable_exits
                        .pop()
                        .expect("recoverable region collector is balanced");
                    debug_assert_eq!(recorded_region, region);
                    flow.continuing = exits.finish(state, flow.continuing);
                }
                Step::IfCondition(owner, expression) => {
                    if flow.continuing {
                        pending.push(Step::IfNext(IfFrame {
                            owner,
                            remaining: [
                                (IfBranch::Then, Some(expression.then_branch)),
                                (IfBranch::Else, expression.else_branch),
                                (IfBranch::Unknown, expression.unknown_branch),
                            ]
                            .into_iter(),
                            branches: Alternatives::begin(state),
                            provenance: ValueProvenance::default(),
                        }));
                    } else {
                        flow = ExpressionFlow::default();
                    }
                }
                Step::IfNext(frame) => {
                    self.schedule_if_branch(frame, state, &mut flow, &mut pending);
                }
                Step::IfValue(mut frame) => {
                    if flow.continuing {
                        frame
                            .provenance
                            .extend(std::mem::take(&mut flow.provenance));
                    }
                    frame.branches.finish_arm(state, flow.continuing);
                    pending.push(Step::IfNext(frame));
                }
                Step::MatchSubject(owner, value) => {
                    if flow.continuing {
                        let key = ExpressionKey::new(self.module, value.scrutinee);
                        let FactState::Known(typed) = self.body_types.expression(key) else {
                            return Err(OwnershipAnalysisError::MissingExpressionType(key));
                        };
                        let subject = if self.signatures.types().contains_reference(typed.ty())? {
                            std::mem::take(&mut flow.provenance)
                        } else {
                            ValueProvenance::default()
                        };
                        pending.push(Step::MatchNext(MatchFrame {
                            owner,
                            remaining: value.arms.iter().enumerate(),
                            branches: Alternatives::begin(state),
                            subject,
                            result: ValueProvenance::default(),
                        }));
                    }
                }
                Step::MatchNext(mut frame) => {
                    if let Some((arm, value)) = frame.remaining.next() {
                        let region = ControlRegionKey::MatchArm {
                            owner: frame.owner,
                            arm,
                        };
                        self.bind_pattern_provenance(value.pattern, &frame.subject, state)?;
                        if let Some(guard) = &value.guard {
                            pending.push(Step::MatchGuard(frame, region, value));
                            pending.push(Step::Expression(guard.condition, None));
                        } else {
                            frame.schedule_value(region, value, &mut pending);
                        }
                    } else {
                        flow = ExpressionFlow {
                            continuing: frame.branches.finish(state),
                            provenance: frame.result,
                        };
                    }
                }
                Step::MatchGuard(mut frame, region, arm) => {
                    // A pattern miss and a false guard can share the next arm's entry
                    // only if they agree on every binding that survives this arm.
                    if flow.continuing {
                        if frame.branches.changed_bindings(state).any(|id| {
                            !matches!(self.body_types.local(id), FactState::Known(local) if local.region() == &region)
                        }) {
                            self.unperformed = true;
                        }
                        frame.schedule_value(region, arm, &mut pending);
                    } else {
                        self.unperformed = true;
                        frame.branches.finish_arm(state, false);
                        pending.push(Step::MatchNext(frame));
                    }
                }
                Step::MatchValue(mut frame, region, origin) => {
                    if flow.continuing {
                        self.check_transfer_source(
                            &ControlTransferSource::RegionCompletion(region),
                            origin,
                            false,
                            state,
                        );
                        frame.result.extend(std::mem::take(&mut flow.provenance));
                    }
                    frame.branches.finish_arm(state, flow.continuing);
                    pending.push(Step::MatchNext(frame));
                }
                Step::BlockNext(mut frame) => {
                    if flow.continuing {
                        if let Some(&statement) = frame.remaining.next() {
                            pending.push(Step::BlockValue(frame));
                            pending.push(Step::Statement(statement));
                            continue;
                        }
                        self.check_transfer_source(
                            &ControlTransferSource::RegionCompletion(frame.region),
                            frame.origin,
                            true,
                            state,
                        );
                    }
                    flow.provenance = frame.provenance;
                }
                Step::BlockValue(mut frame) => {
                    frame
                        .provenance
                        .extend(std::mem::take(&mut flow.provenance));
                    pending.push(Step::BlockNext(frame));
                }
                Step::ValuesNext(mut sequence) => {
                    if (flow.continuing || sequence.visits_after_termination())
                        && let Some(value) = sequence.remaining.next()
                    {
                        pending.push(Step::Value(sequence, value));
                        pending.push(Step::Expression(value, None));
                        continue;
                    }
                    flow.provenance = match sequence.kind {
                        ValueSequenceKind::Call { in_flight } => {
                            self.check_in_flight(&in_flight, state);
                            ValueProvenance::default()
                        }
                        ValueSequenceKind::Tuple { provenance, .. } => provenance,
                        ValueSequenceKind::Array => ValueProvenance::default(),
                        ValueSequenceKind::Discard | ValueSequenceKind::Constructor => {
                            flow.continuing = true;
                            ValueProvenance::default()
                        }
                    };
                }
                Step::Value(mut sequence, value) => {
                    match &mut sequence.kind {
                        ValueSequenceKind::Call { in_flight } => {
                            if flow.continuing {
                                self.recover_missing_move(value, state)?;
                            }
                            self.retain_in_flight(value, &flow.provenance, in_flight)?;
                        }
                        ValueSequenceKind::Tuple { index, provenance } => {
                            if flow.continuing {
                                self.recover_missing_move(value, state)?;
                            }
                            provenance
                                .extend(std::mem::take(&mut flow.provenance).prefixed(*index));
                            *index += 1;
                        }
                        ValueSequenceKind::Array => {
                            if flow.continuing {
                                self.recover_missing_move(value, state)?;
                            }
                        }
                        ValueSequenceKind::Constructor => {
                            self.recover_missing_move(value, state)?;
                        }
                        ValueSequenceKind::Discard => {}
                    }
                    pending.push(Step::ValuesNext(sequence));
                }
                Step::CallCallee(mut sequence, callee) => {
                    let ValueSequenceKind::Call { in_flight } = &mut sequence.kind else {
                        unreachable!("only calls schedule a callee");
                    };
                    self.retain_in_flight(callee, &flow.provenance, in_flight)?;
                    pending.push(Step::ValuesNext(sequence));
                }
                Step::Let(pattern, initializer) => {
                    if flow.continuing {
                        self.bind_let_pattern(pattern, initializer, &flow, state)?;
                    }
                    flow.provenance = ValueProvenance::default();
                }
                Step::AssignmentTarget(target, value) => {
                    pending.push(Step::AssignmentValue(target, flow.continuing));
                    pending.push(Step::Expression(value, None));
                }
                Step::AssignmentValue(target, target_continues) => {
                    if let Some(binding) = self.expression_local(target)?
                        && self.binding_contains_reference(binding)?
                        && flow.continuing
                    {
                        self.record_local_region(binding);
                        state.set_alias(binding, std::mem::take(&mut flow.provenance));
                    }
                    flow = ExpressionFlow {
                        continuing: target_continues && flow.continuing,
                        provenance: ValueProvenance::default(),
                    };
                }
                Step::DiscardProvenance => flow.provenance = ValueProvenance::default(),
                Step::Transfer(key, origin, kind) => {
                    self.check_transfer_source(
                        &ControlTransferSource::Statement(key),
                        origin,
                        false,
                        state,
                    );
                    if !matches!(kind, TransferKind::Yield) || !flow.continuing {
                        flow.provenance = ValueProvenance::default();
                    }
                    flow.continuing = false;
                }
            }
        }
        Ok(flow)
    }

    fn schedule_expression(
        &mut self,
        id: ExprId,
        region_override: Option<ControlRegionKey>,
        state: &mut FlowState,
        flow: &mut ExpressionFlow,
        pending: &mut Vec<Step<'hir>>,
    ) -> Result<(), OwnershipAnalysisError> {
        let expression = self
            .body
            .expression(id)
            .ok_or(OwnershipAnalysisError::WrongBodyExpression(id))?;
        let origin = expression.origin();
        let key = ExpressionKey::new(self.module, id);
        pending.push(Step::FinishExpression(key, origin));
        *flow = ExpressionFlow::continuing();
        match expression.kind() {
            ExprKind::Block { statements } => pending.push(Step::BlockNext(BlockFrame {
                remaining: statements.iter(),
                region: region_override.unwrap_or(ControlRegionKey::Block(key)),
                origin,
                provenance: ValueProvenance::default(),
            })),
            ExprKind::Own(_) | ExprKind::Move(_) | ExprKind::DropOwn { .. } => {
                *flow = self.visit_ownership_operation(key, origin, state)?;
            }
            ExprKind::Name(_) => *flow = self.visit_name(id, origin, state)?,
            ExprKind::Reference { value, .. } => {
                pending.push(Step::Reference(*value));
                pending.push(Step::Expression(*value, None));
            }
            ExprKind::Group(inner) | ExprKind::Unsafe(inner) => {
                pending.push(Step::Expression(*inner, None));
            }
            ExprKind::Unary { operand, .. } => {
                pending.push(Step::Expression(*operand, None));
            }
            ExprKind::Binary { left, right, .. } => pending.push(Step::ValuesNext(
                ValueSequence::new([*left, *right], ValueSequenceKind::Discard),
            )),
            ExprKind::Call(call) => self.schedule_call(
                Some(call.callee),
                call.arguments.iter().map(|argument| argument.value),
                pending,
            ),
            ExprKind::CatalogCall(call) => self.schedule_call(
                None,
                call.arguments.iter().map(|argument| argument.value),
                pending,
            ),
            ExprKind::DTypeCall(call) => self.schedule_call(
                Some(call.callee),
                call.arguments.iter().map(|argument| argument.value),
                pending,
            ),
            ExprKind::Member { receiver, .. } => {
                if !matches!(
                    self.names.fact(ResolutionKey::Expression {
                        module: self.module,
                        expression: id,
                    }),
                    FactState::Known(ResolvedName::TypeMemberOwner(_))
                ) {
                    pending.push(Step::Expression(*receiver, None));
                }
            }
            ExprKind::TupleProjection { receiver, index } => {
                let projection_receiver = if let Some(binding) = self.expression_local(*receiver)?
                    && self.binding_contains_reference(binding)?
                {
                    *flow = ExpressionFlow {
                        continuing: self
                            .expression_continues(ExpressionKey::new(self.module, *receiver), true),
                        provenance: ValueProvenance::default(),
                    };
                    ProjectionReceiver::LocalAlias(binding)
                } else {
                    ProjectionReceiver::Value
                };
                // Alias projections queue no receiver work before their state lookup.
                pending.push(Step::Projection {
                    expression: key,
                    origin,
                    index,
                    receiver: projection_receiver,
                });
                if matches!(projection_receiver, ProjectionReceiver::Value) {
                    pending.push(Step::Expression(*receiver, None));
                }
            }
            ExprKind::Try(operand) => {
                pending.push(Step::DiscardProvenance);
                pending.push(Step::RecoverMove(*operand));
                pending.push(Step::Expression(*operand, None));
            }
            ExprKind::Result(inner) | ExprKind::Checked(inner) => {
                let region = if matches!(expression.kind(), ExprKind::Result(_)) {
                    ControlRegionKey::Result(key)
                } else {
                    ControlRegionKey::Checked(key)
                };
                self.recoverable_exits
                    .push((region.clone(), RecoverableExits::default()));
                pending.push(Step::Recoverable(region.clone()));
                pending.push(Step::Expression(*inner, Some(region)));
            }
            ExprKind::If(value) => {
                pending.push(Step::IfCondition(key, value));
                pending.push(Step::Expression(value.condition, None));
            }
            ExprKind::Match(value) => {
                let subject = self
                    .body_types
                    .expression(ExpressionKey::new(self.module, value.scrutinee));
                if let FactState::Known(typed) = subject
                    && self
                        .signatures
                        .types()
                        .ownership_root(typed.ty())?
                        .is_none()
                {
                    pending.push(Step::MatchSubject(key, value));
                    pending.push(Step::Expression(value.scrutinee, None));
                } else {
                    self.unperformed = true;
                    let children = [value.scrutinee]
                        .into_iter()
                        .chain(value.arms.iter().flat_map(|arm| {
                            arm.guard
                                .iter()
                                .map(|guard| guard.condition)
                                .chain([arm.value])
                        }));
                    pending.push(Step::ValuesNext(ValueSequence::new(
                        children,
                        ValueSequenceKind::Discard,
                    )));
                }
            }
            ExprKind::Tuple(values) => pending.push(Step::ValuesNext(ValueSequence::new(
                values.iter().copied(),
                ValueSequenceKind::Tuple {
                    index: 0,
                    provenance: ValueProvenance::default(),
                },
            ))),
            ExprKind::Array(values) => pending.push(Step::ValuesNext(ValueSequence::new(
                values.iter().copied(),
                ValueSequenceKind::Array,
            ))),
            ExprKind::Map(entries) => pending.push(Step::ValuesNext(ValueSequence::new(
                entries.iter().flat_map(|entry| [entry.key, entry.value]),
                ValueSequenceKind::Discard,
            ))),
            ExprKind::StructLiteral { fields, .. } => {
                pending.push(Step::ValuesNext(ValueSequence::new(
                    fields.iter().map(|field| field.value),
                    ValueSequenceKind::Discard,
                )));
            }
            ExprKind::VariantConstructor { payload, .. } => {
                let values = match payload {
                    ConstructorPayload::Absent => Vec::new(),
                    ConstructorPayload::Tuple(values) => values.to_vec(),
                    ConstructorPayload::Named(fields) => {
                        fields.iter().map(|field| field.value).collect()
                    }
                };
                pending.push(Step::ValuesNext(ValueSequence::new(
                    values,
                    ValueSequenceKind::Constructor,
                )));
            }
            ExprKind::ReduceByCandidate(candidate) => {
                match self.body_types.reduce_by_commitment(key) {
                    FactState::Known(ReduceByCommitment::Ordinary) => {
                        let ordinary = candidate
                            .ordinary
                            .as_ref()
                            .expect("ordinary commitment retains an ordinary candidate");
                        self.schedule_call(
                            Some(ordinary.call.callee),
                            ordinary
                                .call
                                .arguments
                                .iter()
                                .map(|argument| argument.value),
                            pending,
                        );
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
                        pending.push(Step::ValuesNext(ValueSequence::new(
                            [candidate.receiver, value],
                            ValueSequenceKind::Discard,
                        )));
                    }
                    FactState::Known(ReduceByCommitment::TensorPhases) => {
                        let phases = candidate
                            .ordered_phases
                            .as_ref()
                            .expect("phase commitment retains ordered phases");
                        pending.push(Step::ValuesNext(ValueSequence::new(
                            [candidate.receiver]
                                .into_iter()
                                .chain(phases.phases.iter().map(|phase| phase.reducer)),
                            ValueSequenceKind::Discard,
                        )));
                    }
                    FactState::Poisoned(_) | FactState::Unperformed | FactState::NotApplicable => {
                        self.unperformed = true;
                    }
                }
            }
            ExprKind::Lambda(_) => self.unperformed = true,
            ExprKind::Literal(_)
            | ExprKind::Hole
            | ExprKind::Unit
            | ExprKind::TensorSchema(_)
            | ExprKind::Invalid => {}
        }
        Ok(())
    }

    fn schedule_statement(
        &mut self,
        id: StmtId,
        flow: &mut ExpressionFlow,
        pending: &mut Vec<Step<'hir>>,
    ) -> Result<(), OwnershipAnalysisError> {
        let statement = self
            .body
            .statement(id)
            .ok_or(OwnershipAnalysisError::WrongBodyStatement(id))?;
        *flow = ExpressionFlow::continuing();
        match statement.kind() {
            StmtKind::Let {
                pattern,
                initializer,
                ..
            } => {
                if let Some(initializer) = initializer {
                    pending.push(Step::Let(*pattern, *initializer));
                    pending.push(Step::Expression(*initializer, None));
                }
            }
            StmtKind::Assign { target, value } => {
                pending.push(Step::AssignmentTarget(*target, *value));
                if self.expression_local(*target)?.is_some() {
                    flow.continuing =
                        self.expression_continues(ExpressionKey::new(self.module, *target), true);
                } else {
                    pending.push(Step::Expression(*target, None));
                }
            }
            StmtKind::Expression(value) => {
                pending.push(Step::DiscardProvenance);
                pending.push(Step::Expression(*value, None));
            }
            StmtKind::Return(value) => {
                pending.push(Step::Transfer(
                    StatementKey::new(self.module, id),
                    statement.origin(),
                    TransferKind::Return,
                ));
                if let Some(value) = value {
                    pending.push(Step::RecoverMove(*value));
                    pending.push(Step::Expression(*value, None));
                }
            }
            StmtKind::Yield(value) => {
                pending.push(Step::Transfer(
                    StatementKey::new(self.module, id),
                    statement.origin(),
                    TransferKind::Yield,
                ));
                pending.push(Step::RecoverMove(*value));
                pending.push(Step::Expression(*value, None));
            }
            StmtKind::While { condition, body } => {
                self.unperformed = true;
                pending.push(Step::ValuesNext(ValueSequence::new(
                    [*condition, *body],
                    ValueSequenceKind::Discard,
                )));
            }
            StmtKind::For { iterable, body, .. } => {
                self.unperformed = true;
                pending.push(Step::ValuesNext(ValueSequence::new(
                    [*iterable, *body],
                    ValueSequenceKind::Discard,
                )));
            }
            StmtKind::Break | StmtKind::Continue => {
                self.unperformed = true;
                *flow = ExpressionFlow::default();
            }
            StmtKind::Invalid => {}
        }
        Ok(())
    }

    fn schedule_call(
        &self,
        callee: Option<ExprId>,
        arguments: impl IntoIterator<Item = ExprId>,
        pending: &mut Vec<Step<'hir>>,
    ) {
        let sequence = ValueSequence::new(
            arguments,
            ValueSequenceKind::Call {
                in_flight: Vec::new(),
            },
        );
        if let Some(callee) = callee {
            pending.push(Step::CallCallee(sequence, callee));
            pending.push(Step::Expression(callee, None));
        } else {
            pending.push(Step::ValuesNext(sequence));
        }
    }

    fn schedule_if_branch(
        &self,
        mut frame: IfFrame,
        state: &mut FlowState,
        flow: &mut ExpressionFlow,
        pending: &mut Vec<Step<'hir>>,
    ) {
        for (branch, value) in frame.remaining.by_ref() {
            let Some(value) = value else {
                if matches!(branch, IfBranch::Else) {
                    frame.branches.unchanged_path();
                }
                continue;
            };
            let region = ControlRegionKey::IfBranch {
                owner: frame.owner,
                branch,
            };
            pending.push(Step::IfValue(frame));
            pending.push(Step::Expression(value, Some(region)));
            return;
        }
        *flow = ExpressionFlow {
            continuing: frame.branches.finish(state),
            provenance: frame.provenance,
        };
    }
}
