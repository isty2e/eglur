use super::application::{ApplicationContinuation, ApplicationPlan};
use super::branching::{BranchContinuation, BranchPlan};
use super::expression::ExpressionPlan;
use super::failure::{CheckedFrame, FailurePlan, QuestionFrame, ResultFrame};
use super::invocation::{InvocationContinuation, InvocationFrame, InvocationPlan};
use super::numeric::{NumericContinuation, NumericPlan};
use super::numeric_tree::{NumericTreeFrame, NumericTreePlan};
use super::statements::{BlockFrame, StatementContinuation, StatementPlan};
use super::values::{ValueContinuation, ValuePlan};
use super::*;

enum Cleanup {
    Region(ControlRegionKey),
    ResultContext,
    CheckedContext,
    QuestionFloor,
    BlockOverride(ExprId),
    NumericPreparation(bool),
    PathBarrier(Option<usize>),
}

enum Step {
    Expression(ExprId, TypeRequest),
    InCurrentRegion(ExprId, TypeRequest),
    Publish(ExpressionKey),
    Block(BlockFrame),
    Statement(StmtId, TypeRequest),
    ResumeStatement(StatementContinuation),
    ResumeBlock(BlockFrame),
    Value(Box<ValuePlan>),
    ResumeValue(ValueContinuation),
    Invocation(Box<InvocationPlan>),
    ResumeApplication {
        continuation: ApplicationContinuation,
        invocation: Box<InvocationFrame>,
    },
    ResumeInvocation(InvocationContinuation),
    Branch(Box<BranchPlan>),
    ResumeBranch(BranchContinuation),
    ResumeMatchArm,
    NormalCompletion(ControlRegionKey),
    NumericTree(Box<NumericTreeFrame>),
    ResumeNumericTree(Box<NumericTreeFrame>),
    Numeric(Box<NumericPlan>),
    ResumeNumeric(NumericContinuation),
    Failure(Box<FailurePlan>),
    FinishResult(ResultFrame),
    FinishChecked(CheckedFrame),
    FinishQuestion(QuestionFrame),
    Cleanup(Cleanup),
}

struct ScopedStep {
    step: Step,
    work: Option<crate::AnalysisRootTicket>,
}

struct ScopedSteps<'a> {
    pending: &'a mut Vec<ScopedStep>,
    work: Option<&'a crate::AnalysisRootTicket>,
}

impl ScopedSteps<'_> {
    fn push(&mut self, step: Step) {
        self.pending.push(ScopedStep {
            step,
            work: self.work.cloned(),
        });
    }
}

impl<'input, 'hir, 'support> BodyChecker<'input, 'hir, 'support> {
    pub(super) fn type_expression_steps(
        &mut self,
        context: BodyContext<'_, '_>,
        expression: ExprId,
        request: TypeRequest,
    ) -> Result<Outcome<TypedExpression>, BodyFlow> {
        self.execute_body_steps(context, vec![Step::Expression(expression, request)])
    }

    pub(super) fn type_expression_in_current_region(
        &mut self,
        module: ModuleId,
        source: &SemanticSource<'_>,
        hir: &HirFile,
        body: &Body,
        expression: ExprId,
        request: TypeRequest,
    ) -> Result<Outcome<TypedExpression>, BodyFlow> {
        self.execute_body_steps(
            BodyContext {
                module,
                source,
                hir,
                body,
                work: None,
            },
            vec![Step::InCurrentRegion(expression, request)],
        )
    }

    fn execute_body_steps(
        &mut self,
        context: BodyContext<'_, '_>,
        pending: Vec<Step>,
    ) -> Result<Outcome<TypedExpression>, BodyFlow> {
        let mut pending = pending
            .into_iter()
            .map(|step| ScopedStep {
                step,
                work: context.work.cloned(),
            })
            .collect();
        let result = self.run_body_steps(context, &mut pending);
        let Err(error) = result else {
            return result;
        };

        // Unwind lexical scopes without publishing or resuming failed work.
        #[cfg(test)]
        execution::tests::sweep_support::before_unwind(self);
        while let Some(ScopedStep { step, .. }) = pending.pop() {
            if let Step::Cleanup(cleanup) = step
                && let Err(cleanup_error) = self.finish_typing_scope(cleanup)
            {
                self.retain_cleanup_failure(cleanup_error);
            }
        }
        Err(error)
    }

    fn run_body_steps(
        &mut self,
        context: BodyContext<'_, '_>,
        pending: &mut Vec<ScopedStep>,
    ) -> Result<Outcome<TypedExpression>, BodyFlow> {
        let mut expression_value = None;
        let mut statement_value = None;
        let mut completed_match_arm = None;
        let mut completed_result_context = None;
        while let Some(ScopedStep { mut step, mut work }) = pending.pop() {
            if !matches!(step, Step::Cleanup(_)) {
                self.ensure_analysis_active()?;
            }
            if let Step::Invocation(plan) = &step
                && let Some(selection) = plan.analysis_work()
            {
                work = Some(selection.clone());
            }
            match &mut step {
                Step::Expression(_, request)
                | Step::InCurrentRegion(_, request)
                | Step::Statement(_, request) => {
                    if request.work.is_none()
                        && matches!(request.custody(), ExpectationCustody::Caller)
                    {
                        request.work = work.clone();
                    }
                    *request = self.attach_request_work(context.source.file(), request.clone())?;
                    work = request.work.clone();
                }
                _ => {}
            }
            let context = BodyContext {
                work: work.as_ref(),
                ..context
            };
            #[cfg(test)]
            {
                use expectation::tests::{Event, ResumeKind, observe};
                let kind = match &step {
                    Step::ResumeValue(_) => Some(ResumeKind::Value),
                    Step::ResumeApplication { .. } => Some(ResumeKind::Application),
                    Step::ResumeInvocation(_) => Some(ResumeKind::Invocation),
                    Step::ResumeStatement(_) => Some(ResumeKind::Statement),
                    Step::ResumeBranch(_) => Some(ResumeKind::Branch),
                    Step::ResumeNumeric(_) => Some(ResumeKind::Numeric),
                    Step::ResumeNumericTree(_) => Some(ResumeKind::NumericTree),
                    Step::FinishResult(_) => Some(ResumeKind::Result),
                    Step::FinishChecked(_) => Some(ResumeKind::Checked),
                    Step::FinishQuestion(_) => Some(ResumeKind::Question),
                    _ => None,
                };
                if let Some(kind) = kind {
                    observe(Event::Resume {
                        owner: self.current_owner,
                        kind,
                        work: work.clone(),
                    });
                }
            }
            let mut scoped = ScopedSteps {
                pending,
                work: work.as_ref(),
            };
            let pending = &mut scoped;
            match step {
                Step::Expression(expression, request) => {
                    match self.prepare_expression(context, expression, request)? {
                        ExpressionPlan::Complete(outcome) => expression_value = Some(outcome),
                        ExpressionPlan::Publish { key, outcome } => {
                            expression_value = Some(outcome);
                            pending.push(Step::Publish(key));
                        }
                        ExpressionPlan::Block {
                            key,
                            statements,
                            request,
                        } => {
                            pending.push(Step::Publish(key));
                            if self.block_region_overrides.last() != Some(&expression) {
                                let region = ControlRegionKey::Block(key);
                                self.enter_control_region(region.clone())?;
                                pending.push(Step::Cleanup(Cleanup::Region(region)));
                            }
                            pending.push(Step::Block(BlockFrame::new(statements, request)));
                        }
                        ExpressionPlan::Forward {
                            key,
                            expression,
                            request,
                        } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Expression(expression, request));
                        }
                        ExpressionPlan::Value { key, plan } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Value(plan));
                        }
                        ExpressionPlan::Branch { key, plan } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Branch(plan));
                        }
                        ExpressionPlan::NumericTree(frame) => {
                            let previous =
                                std::mem::replace(&mut self.preparing_numeric_tree, true);
                            pending.push(Step::Cleanup(Cleanup::NumericPreparation(previous)));
                            pending.push(Step::NumericTree(frame));
                        }
                        ExpressionPlan::Numeric { key, plan } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Numeric(plan));
                        }
                        ExpressionPlan::Failure { key, plan } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Failure(plan));
                        }
                        ExpressionPlan::Invocation { key, plan } => {
                            pending.push(Step::Publish(key));
                            pending.push(Step::Invocation(plan));
                        }
                    }
                }
                Step::InCurrentRegion(expression, request) => {
                    let is_block = matches!(
                        context
                            .body
                            .expression(expression)
                            .ok_or(BodyTypingError::WrongBodyExpression(expression))?
                            .kind(),
                        ExprKind::Block { .. }
                    );
                    if is_block {
                        self.block_region_overrides.push(expression);
                        pending.push(Step::Cleanup(Cleanup::BlockOverride(expression)));
                    } else {
                        pending.push(Step::NormalCompletion(
                            self.current_control_region()?.clone(),
                        ));
                    }
                    pending.push(Step::Expression(expression, request));
                }
                Step::Publish(key) => {
                    let outcome = expression_value
                        .as_ref()
                        .expect("expression publication follows its evaluation");
                    self.record_expression(key, outcome.clone())?;
                }
                Step::Block(mut block) => {
                    if let Some((statement, request, path)) = block.next_statement() {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeBlock(block));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::Statement(statement, request));
                    } else {
                        expression_value = Some(self.finish_block(block)?);
                    }
                }
                Step::Statement(statement, request) => {
                    let plan = self.prepare_statement(context, statement, request)?;
                    self.schedule_statement(plan, &mut statement_value, pending);
                }
                Step::ResumeStatement(continuation) => {
                    let value = expression_value
                        .take()
                        .expect("statement continuation follows its child expression");
                    let plan = self.resume_statement(context, continuation, value)?;
                    self.schedule_statement(plan, &mut statement_value, pending);
                }
                Step::ResumeBlock(block) => {
                    let value = statement_value
                        .take()
                        .expect("block continuation follows its statement");
                    pending.push(Step::Block(self.resume_block(block, value)?));
                }
                Step::Value(plan) => match *plan {
                    ValuePlan::Complete(outcome) => expression_value = Some(outcome),
                    ValuePlan::Expression {
                        expression,
                        request,
                        path,
                        continuation,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeValue(continuation));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::Expression(expression, request));
                    }
                },
                Step::ResumeValue(continuation) => {
                    let outcome = expression_value
                        .take()
                        .expect("value continuation follows its child expression");
                    let plan = self.resume_value(context, continuation, outcome)?;
                    pending.push(Step::Value(Box::new(plan)));
                }
                Step::Invocation(plan) => match *plan {
                    InvocationPlan::Complete(outcome) => expression_value = Some(outcome),
                    InvocationPlan::Application { plan, continuation } => {
                        self.schedule_application(
                            context,
                            plan,
                            continuation,
                            &mut expression_value,
                            pending,
                        )?;
                    }
                    InvocationPlan::Expression {
                        expression,
                        request,
                        path,
                        continuation,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeInvocation(continuation));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::Expression(expression, request));
                    }
                },
                Step::ResumeApplication {
                    continuation,
                    invocation,
                } => {
                    let outcome = expression_value
                        .take()
                        .expect("application continuation follows its child expression");
                    let plan = self.resume_application(context.source, continuation, outcome)?;
                    self.schedule_application(
                        context,
                        plan,
                        invocation,
                        &mut expression_value,
                        pending,
                    )?;
                }
                Step::ResumeInvocation(continuation) => {
                    let outcome = expression_value
                        .take()
                        .expect("invocation continuation follows its child expression");
                    let plan = self.resume_invocation(context, continuation, outcome)?;
                    pending.push(Step::Invocation(Box::new(plan)));
                }
                Step::Branch(plan) => match *plan {
                    BranchPlan::Complete(outcome) => expression_value = Some(outcome),
                    BranchPlan::Expression {
                        expression,
                        request,
                        continuation,
                        path,
                    } => {
                        pending.push(Step::ResumeBranch(continuation));
                        if let Some(floor) = self.push_control_path_barrier(path) {
                            pending.push(Step::Cleanup(Cleanup::PathBarrier(Some(floor))));
                        }
                        pending.push(Step::Expression(expression, request));
                    }
                    BranchPlan::IfBranch {
                        region,
                        path,
                        expression,
                        request,
                        frame,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeBranch(BranchContinuation::IfBranch(frame)));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        self.enter_control_region(region.clone())?;
                        pending.push(Step::Cleanup(Cleanup::Region(region)));

                        pending.push(Step::InCurrentRegion(expression, request));
                    }
                    BranchPlan::MatchArm {
                        region,
                        path,
                        frame,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeMatchArm);
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        self.enter_control_region(region.clone())?;
                        pending.push(Step::Cleanup(Cleanup::Region(region)));
                        let plan = self.prepare_match_arm(context, frame)?;
                        pending.push(Step::Branch(Box::new(plan)));
                    }
                    BranchPlan::MatchBody {
                        region,
                        path,
                        expression,
                        request,
                        frame,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeBranch(BranchContinuation::MatchValue {
                            frame,
                            guard_continuation: path,
                        }));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::NormalCompletion(region));
                        pending.push(Step::Expression(expression, request));
                    }
                    BranchPlan::CompleteMatchArm(frame) => {
                        assert!(completed_match_arm.replace(frame).is_none());
                    }
                },
                Step::ResumeBranch(continuation) => {
                    let outcome = expression_value
                        .take()
                        .expect("branch continuation follows its child expression");
                    let plan = self.resume_branch(context, continuation, outcome)?;
                    pending.push(Step::Branch(Box::new(plan)));
                }
                Step::ResumeMatchArm => {
                    let frame = completed_match_arm
                        .take()
                        .expect("match advances after arm completion and scope cleanup");
                    let plan = self.advance_match(context.source, frame)?;
                    pending.push(Step::Branch(Box::new(plan)));
                }
                Step::NormalCompletion(region) => {
                    if matches!(
                        expression_value.as_ref(),
                        Some(Outcome::Known(value)) if value.normal_continuation.is_present()
                    ) {
                        self.record_control_transfer(
                            ControlTransferSource::RegionCompletion(region.clone()),
                            ControlTransferKind::NormalCompletion,
                            region,
                        )?;
                    }
                }
                Step::NumericTree(frame) => match self.advance_numeric_tree(context, frame)? {
                    NumericTreePlan::Complete(outcome) => expression_value = Some(outcome),
                    NumericTreePlan::Expression {
                        expression,
                        request,
                        path,
                        finishing,
                        frame,
                    } => {
                        let previous =
                            std::mem::replace(&mut self.preparing_numeric_tree, finishing);
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeNumericTree(frame));
                        pending.push(Step::Cleanup(Cleanup::NumericPreparation(previous)));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::Expression(expression, request));
                    }
                },
                Step::ResumeNumericTree(mut frame) => {
                    let outcome = expression_value
                        .take()
                        .expect("numeric tree resumes after its child expression");
                    frame.accept_child(outcome);
                    pending.push(Step::NumericTree(frame));
                }
                Step::Numeric(plan) => match *plan {
                    NumericPlan::Complete(outcome) => expression_value = Some(outcome),
                    NumericPlan::Expression {
                        expression,
                        request,
                        path,
                        continuation,
                    } => {
                        let barrier = self.push_control_path_barrier(path);
                        pending.push(Step::ResumeNumeric(continuation));
                        pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                        pending.push(Step::Expression(expression, request));
                    }
                },
                Step::ResumeNumeric(continuation) => {
                    let outcome = expression_value
                        .take()
                        .expect("numeric operation resumes after its operand");
                    let plan = self.resume_numeric(context.source, continuation, outcome)?;
                    pending.push(Step::Numeric(Box::new(plan)));
                }
                Step::Failure(plan) => match *plan {
                    FailurePlan::Complete(outcome) => expression_value = Some(outcome),
                    FailurePlan::ResultBlock {
                        block,
                        request,
                        context: result_context,
                        frame,
                    } => {
                        pending.push(Step::FinishResult(frame));
                        let region = result_context.region.clone();
                        self.enter_control_region(region.clone())?;
                        pending.push(Step::Cleanup(Cleanup::Region(region)));
                        self.result_contexts.push(result_context);
                        pending.push(Step::Cleanup(Cleanup::ResultContext));
                        pending.push(Step::InCurrentRegion(block, request));
                    }
                    FailurePlan::CheckedBlock {
                        block,
                        request,
                        context: checked_context,
                        frame,
                    } => {
                        pending.push(Step::FinishChecked(frame));
                        let region = checked_context.region.clone();
                        self.enter_control_region(region.clone())?;
                        pending.push(Step::Cleanup(Cleanup::Region(region)));
                        self.question_floors.push(self.result_contexts.len());
                        pending.push(Step::Cleanup(Cleanup::QuestionFloor));
                        self.checked_contexts.push(checked_context);
                        pending.push(Step::Cleanup(Cleanup::CheckedContext));
                        pending.push(Step::InCurrentRegion(block, request));
                    }
                    FailurePlan::QuestionOperand {
                        operand,
                        request,
                        frame,
                    } => {
                        pending.push(Step::FinishQuestion(frame));
                        pending.push(Step::Expression(operand, request));
                    }
                },
                Step::FinishResult(frame) => {
                    let result_context = completed_result_context
                        .take()
                        .expect("result finalization follows context exit");
                    let outcome = expression_value
                        .take()
                        .expect("result finalization follows its body");
                    expression_value =
                        Some(self.finish_result(context, frame, result_context, outcome)?);
                }
                Step::FinishChecked(frame) => {
                    let outcome = expression_value
                        .take()
                        .expect("checked finalization follows its body");
                    expression_value = Some(self.finish_checked(context, frame, outcome)?);
                }
                Step::FinishQuestion(frame) => {
                    let outcome = expression_value
                        .take()
                        .expect("question finalization follows its operand");
                    expression_value =
                        Some(self.finish_question(context.source, frame, outcome)?);
                }
                Step::Cleanup(cleanup) => {
                    if let Some(result_context) = self.finish_typing_scope(cleanup)? {
                        assert!(completed_result_context.replace(result_context).is_none());
                    }
                }
            }
        }
        Ok(expression_value.expect("expression work produces its final result"))
    }

    fn schedule_application(
        &mut self,
        context: BodyContext<'_, '_>,
        plan: ApplicationPlan,
        invocation: Box<InvocationFrame>,
        value: &mut Option<Outcome<TypedExpression>>,
        pending: &mut ScopedSteps<'_>,
    ) -> Result<(), BodyFlow> {
        match plan {
            ApplicationPlan::Complete(outcome) => {
                *value = Some(self.finish_invocation(context, *invocation, outcome)?);
            }
            ApplicationPlan::Expression {
                expression,
                request,
                path,
                continuation,
            } => {
                let barrier = self.push_control_path_barrier(path);
                pending.push(Step::ResumeApplication {
                    continuation,
                    invocation,
                });
                pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                pending.push(Step::Expression(expression, request));
            }
        }
        Ok(())
    }

    fn schedule_statement(
        &mut self,
        plan: StatementPlan,
        value: &mut Option<Outcome<BlockStep>>,
        pending: &mut ScopedSteps<'_>,
    ) {
        let work = match &plan {
            StatementPlan::Expression { request, .. } => {
                request.work.clone().or_else(|| pending.work.cloned())
            }
            StatementPlan::Complete(_) => pending.work.cloned(),
        };
        let mut pending = ScopedSteps {
            pending: &mut *pending.pending,
            work: work.as_ref(),
        };
        match plan {
            StatementPlan::Complete(outcome) => *value = Some(outcome),
            StatementPlan::Expression {
                expression,
                request,
                path,
                continuation,
            } => {
                let barrier = self.push_control_path_barrier(path);
                pending.push(Step::ResumeStatement(continuation));
                pending.push(Step::Cleanup(Cleanup::PathBarrier(barrier)));
                pending.push(Step::Expression(expression, request));
            }
        }
    }

    fn finish_typing_scope(
        &mut self,
        cleanup: Cleanup,
    ) -> Result<Option<ResultContext>, BodyTypingError> {
        match cleanup {
            Cleanup::ResultContext => {
                return self
                    .result_contexts
                    .pop()
                    .map(Some)
                    .ok_or(BodyTypingError::MissingResultContext);
            }
            Cleanup::CheckedContext => self
                .checked_contexts
                .pop()
                .map(|_| ())
                .ok_or(BodyTypingError::MissingCheckedContext),
            Cleanup::QuestionFloor => self
                .question_floors
                .pop()
                .map(|_| ())
                .ok_or(BodyTypingError::MissingQuestionBoundary),

            Cleanup::NumericPreparation(previous) => {
                self.preparing_numeric_tree = previous;
                Ok(())
            }
            Cleanup::Region(region) => self.leave_control_region(&region),
            Cleanup::BlockOverride(expression) => self.pop_block_region_override(expression),
            Cleanup::PathBarrier(barrier) => self.pop_control_path_barrier(barrier),
        }?;
        Ok(None)
    }
}
