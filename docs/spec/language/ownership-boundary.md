# Ownership Boundary

Scope: Normative for v0.1 unless this file or a section states otherwise.

### v0.1 Ownership Island

`Own[T]` marks an explicit exact-once handoff boundary. It is not a general
linearity qualifier for arbitrary aggregate values in v0.1.

The following terms are normative:

`calls-references.md` defines **reference-free type**; the definitions below use that alias-expanded structural classification.

1. An **ownership-free type** contains no `Own[...]` after transparent alias expansion.
2. A **direct ownership-root type** is `Own[T]`, where `T` is ownership-free and reference-free.
3. A **conditional ownership-root type** is exactly one of:
   1. `Opt[Own[T]]`, where `T` is ownership-free and reference-free,
   2. `Res[Own[T], E]`, where both `T` and `E` are ownership-free and reference-free.
4. An **ownership-root type** is either a direct or conditional ownership-root type.
5. `&R` and `&mut R`, where `R` is an ownership-root type, are non-owning borrow views and do not themselves create ownership roots.
6. A **resource payload type** is an opaque standard-library type explicitly designated as owning an external mutable resource.
7. A **resource root** is `Own[R]`, `Opt[Own[R]]`, or `Res[Own[R], E]` where `R` is a reference-free resource payload type and `E` is ownership-free and reference-free.

Rules:

1. Ownership surface (`Own[T]`, `own`, `move`, and `drop_own`) is baseline-required for v0.1 conformance.
2. `Own[T]` is a language-intrinsic core type constructor:
   1. `std::prelude` MUST re-export the same intrinsic symbol,
   2. implementations MUST NOT substitute a distinct wrapper or alias type.
3. Ownership-root types MAY appear only as complete named function/method parameter types, complete return types, and complete local-binding types.
4. The two conditional root shapes in the definition above are the only v0.1 aggregate exceptions.
5. Every other placement that contains `Own[...]` MUST fail with `OWNERSHIP_TYPE_POSITION_FORBIDDEN`, including:
   1. user `struct` fields and user `enum` payloads,
   2. tuple elements,
   3. collection, `Box`, or other ordinary generic arguments,
   4. callable parameter or result types nested inside `Fn(...) -> ...`,
   5. `Res` error payloads,
   6. nested or repeated carrier layers such as `Opt[Opt[Own[T]]]`,
   7. transparent alias targets that would hide an ownership root,
   8. an `Own[T]` whose `T` is not ownership-free or reference-free.
6. Ordinary generic parameters range only over ownership-free types in v0.1 and, independently, over reference-free types under `calls-references.md`.
   An ownership root may be abstracted only by writing the wrapper in the declaration, for example `fn pass[T](x: Own[T]) -> Own[T]`.
7. Calls or explicit type arguments MUST NOT instantiate an ordinary generic parameter with an ownership-root type (`OWNERSHIP_TYPE_POSITION_FORBIDDEN` otherwise).
   When one argument violates both the ownership and reference-free restrictions, this ownership diagnostic is primary.
8. v0.1 does not support ownership-bearing user aggregates, ownership-bearing ordinary containers, partial-field moves, or ownership-capable callable values.
9. The excluded positions in Rule 8 MUST NOT become ownership-capable merely because a type contains an `Own` leaf.
10. A bang marker grants effect permission only; it does not create, transfer, or weaken ownership obligations.
11. `Own[T]` MUST NOT be interpreted as effect authorization or as a recursive-layout indirection boundary.
12. Method lookup on `Own[T]` uses the wrapper's static type and MUST NOT forward inherent methods or trait implementations from `T`.
13. `Own[T]` does not implement `Clone` in v0.1; `x.clone()` on an ownership root MUST fail with `METHOD_NOT_FOUND` rather than duplicate the root.
14. `Own[T]` does not implement `Eq` in v0.1 and MUST NOT forward an `Eq` implementation from `T`; comparing ownership roots fails with `COMPARISON_TYPE_MISMATCH`.
15. No implicit destruction operation discharges a root.
    For non-resource roots, the only explicit discard operation is `drop_own move <identifier>`.
16. A closure or partial application MUST NOT capture an ownership root or a reference derived from one; such an ownership-capable callable value MUST fail with `OWNERSHIP_TYPE_POSITION_FORBIDDEN`.
17. Rule 16 applies even when the callable is passed directly to a callee; v0.1 does not infer callback invocation count or lifetime to relax the closed callable boundary.
18. Code that needs ownership transfer through reusable logic MUST use a direct call to a named function with an explicit ownership-root parameter.
19. Standard-library modules MAY declare methods directly on specific `Own[R]` instantiations where `R` is a resource payload type.
    This is method lookup on the wrapper's static type, not forwarding from `R`.
20. A consuming resource terminal such as `Own[R].close!(self: Self)` discharges its resource root before returning either `Ok` or `Err`.
21. `drop_own move x` MUST NOT accept a resource root; attempts MUST fail with `RESOURCE_DROP_FORBIDDEN` because releasing an external resource is an effectful operation that requires an explicit bang boundary.
22. A standard-library scoped resource API MAY create and discharge an internal resource root while exposing only a non-escaping `&mut Own[R]` callback parameter.
    That callback parameter is an API-level scoped writable reference, not an ownership-bearing first-class callable type or capture environment.
23. `&Own[T]` and `&mut Own[T]` are aliasable local reference values under
    `calls-references.md`; neither proves unique access to the root. A named
    function that requires unique exact-once responsibility MUST take
    `Own[T]` by value, and a caller transferring a named root MUST write
    `move root`.

### Root Creation and Transfer

A binding of ownership-root type establishes an ownership obligation. For a
consuming pattern, that obligation begins when the arm is selected, after its guard.
A fresh ownership-bearing temporary produced by `own`, a call, or a constructor
MUST be immediately established as a root, transferred to an allowed result or
carrier, or consumed by the carrier operations below.

Rules:

1. `own <identifier>` requires an available ownership-free local binding or function/method parameter of type `T`.
2. A resolved const or module/global binding is not a consumable `own` source and MUST fail with `OWN_REQUIRES_LOCAL_BINDING`; unresolved names use ordinary name-resolution diagnostics first.
3. Successful `own` consumes its source binding and produces a fresh `Own[T]` root value.
4. The source binding in Rule 3 becomes unavailable; subsequent use MUST fail with `OWNERSHIP_USE_AFTER_DISCHARGE`.
5. Ownership-free copies created before `own` remain independent ordinary values; `own` does not retroactively invalidate other bindings.
6. Applying `own` to any ownership-root type MUST fail with `OWN_REQUIRES_NON_OWN`.
7. `move <identifier>` consumes an available named ownership root and produces the same ownership-root type.
8. Moving from a direct root yields `Own[T]`; moving from a conditional root preserves its complete `Opt[Own[T]]` or `Res[Own[T], E]` type.
9. A named root MUST use `move` whenever ownership transfers to:
   1. another local root,
   2. a by-value ownership-root parameter,
   3. a function or block result,
   4. an allowed `Some(...)` or `Ok(...)` carrier payload,
   5. a consuming `match` scrutinee.
10. A consuming use of a named root without `move` MUST fail with `OWNERSHIP_TRANSFER_REQUIRES_MOVE`.
11. A fresh ownership-bearing temporary has no prior named root and MAY initialize or return an ownership-root value directly.
12. `move` is valid only for an identifier whose static type is an ownership-root type; other operands MUST fail with `MOVE_REQUIRES_OWN`.
13. `move(<expr>)` is not ownership syntax in v0.1.
14. Borrowing with `&x` or `&mut x` does not transfer or unwrap `Own[T]`; an API that borrows ownership MUST declare `&Own[T]` or `&mut Own[T]` explicitly.
15. A reference derived from an ownership root retains that root as provenance for ownership-flow analysis.
16. Provenance follows local reference copies, rebindings, and control-flow merges; a merged reference value retains every root that may produce that value on the current typed path.
17. Consuming a root invalidates every currently derived reference for later use; a reference use is valid only when every root in its current provenance set is definitely `Owned`.
18. Violating Rule 17 MUST fail with `OWNERSHIP_USE_AFTER_DISCHARGE`.
19. Rule 17 is use-sensitive rather than lexical: consuming a root after the last use of a derived reference remains valid.
20. Replacing an ownership root through assignment, including through `&mut`, would discard the previous obligation and MUST fail with `OWNERSHIP_DISCARD_FORBIDDEN`.
21. For `own x`, diagnostic precedence is: disabled profile, name resolution, ownership-root operand, prior consumption, then local-binding eligibility.
22. A successful by-value transfer establishes the callee parameter as the one
    available ownership root for that value in source semantics. Derived
    references do not transfer ownership and MUST NOT be used through the same
    or a later call after their root is consumed.
23. Lexical shadowing changes name lookup only. A hidden ownership root keeps
    its binding identity, path-sensitive state, provenance, and obligation
    region; introducing or discharging a same-spelled inner binding does not
    consume, transfer, or discharge that outer root.

Example:

```eglur
fn pass[T](x: Own[T]) -> Own[T] {
  return move x
}

fn inspect[T](_x: &Own[T]) -> Unit {
}
```

### Conditional Root Carriers

`Opt[Own[T]]` and `Res[Own[T], E]` carry at most one direct ownership root.
They are built-in control-flow exceptions, not general ownership-capable generic
instantiations.

Rules:

1. `Some(move x)` and `Ok(move x)` transfer an existing direct root into a fresh conditional root.
2. A fresh direct-root expression MAY be passed to `Some(...)` or `Ok(...)` without an additional `move` marker.
3. `None` and `Err(e)` carry no ownership payload, but a binding of conditional root type remains one root value that MUST be consumed or transferred exactly once.
4. Applying `?` to a named conditional root requires `(move r)?`.
5. Applying `?` directly to a fresh conditional-root expression, for example `open!()?`, is valid without an intermediate `move`.
6. On `Some`/`Ok`, `?` establishes the payload as a fresh direct root in the receiving binding.
7. On `None`/`Err`, `?` consumes the carrier and performs the ordinary typed early exit; no direct root is created.
8. Consuming pattern extraction requires `match move r { ... }` for a named carrier.
9. A successful `Some(x)` or `Ok(x)` binding in Rule 8 introduces `x` as a direct ownership root in the selected arm; that arm MUST discharge or transfer `x` before an edge exits the arm's obligation region.
   1. Each complete pattern alternative for a conditional root, named or fresh,
      MUST bind its ownership at most once (`PATTERN_OWNERSHIP_BINDING_OVERLAP`
      otherwise). Both `whole @ Some(x)` and `Some(x @ y)` bind it twice. The
      rule applies before the guard, even if the arm never uses one of the names.
   2. A pattern may bind just the carrier (`whole @ Some(_)`) or just its
      payload (`Some(x)`). The selected arm acquires the root at the type it
      bound. In `whole @ Err(reason)`, only `whole` owns a root; `reason` is
      ownership-free.
   3. An arm that can select a present payload MUST NOT discard it by binding
      neither the carrier nor the payload (`OWNERSHIP_DISCARD_FORBIDDEN`).
      This includes `Some(_)` and `Ok(..)` without an enclosing owning binding.
      Selecting `None` or `Err(e)` without binding the carrier creates no root.
10. Pattern bindings are tentative while a guard is evaluated:
   1. a tentative ownership binding MAY be used only through a non-escaping shared borrow,
   2. `move`, `drop_own`, by-value transfer, and every other consuming use of a tentative ownership binding MUST fail with `OWNERSHIP_GUARD_CONSUME_FORBIDDEN`,
   3. a guard-false path retains the complete moved scrutinee inside match evaluation for later arms,
   4. a guard-true path ends any guard borrow before the selected arm acquires the root bound under Rule 9.
   5. If a typed control-flow edge leaves the match during a guard and that
      arm can select a present payload under Rule 9.3, compilation MUST fail
      with `OWNERSHIP_PATH_UNBALANCED` at the exiting construct. Use the
      current pattern and the coverage of preceding unguarded arms; a guard
      that evaluates to false does not rule out that variant.
   6. Apply Rule 10.5 to each match the edge leaves during guard evaluation.
      For example, returning from a `whole @ None` guard leaves no payload
      behind, but the same return still fails if it also leaves a pending
      `Some(x)` guard. A `?` caught by a `result` inside the guard does not
      leave that match. Other roots follow the ordinary region-exit rules.
11. Move-binding patterns such as `Some(move x)` are not part of the v0.1 pattern surface.
12. Borrow-only carrier queries whose receiver is `&Self` MAY inspect a conditional root without consuming it.
13. Ordinary by-value generic adapters on `Opt` or `Res` are not available when their type arguments form a conditional ownership root unless this section explicitly defines the operation.

### Exact-once Discharge

Every ownership root in a bang or non-bang function/method body belongs to one
lexical **obligation region**. It MUST be discharged exactly once before a
typed control-flow edge exits that region, unless the edge explicitly transfers
the root into its result or another allowed root.

Discharge points are:

1. `move` into another allowed root, parameter, carrier, or typed result,
2. `(move r)?` or `match move r` for a conditional root,
3. `drop_own move x` for a non-resource direct or conditional root,
4. a consuming standard-library resource terminal explicitly declared for the root's static type.

Rules:

1. The function/method body is the outermost obligation region for its
   parameters and body-level local roots. Every nested ordinary block, `if`
   branch, `match` arm, loop body, `result` body, and `checked` body introduces
   a nested obligation region for roots bound directly within that region.
   Closure bodies are analyzed as separate callables under the ownership-free
   capture rules.
2. A local binding that receives a fresh or transferred ownership value
   establishes a new root in the binding's containing obligation region after
   its initializer completes. A successful consuming-pattern binding
   establishes its root in the selected match-arm region.
3. Moving a root into a block result, function result, parameter, carrier, or
   another local root discharges the source obligation. When a receiving local
   binding is created, that binding owns a new obligation in its own region.
   A transfer does not retain the source region on the destination root.
4. For each typed control-flow edge, implementations MUST compute the finite
   set of active obligation regions that the edge exits. After accounting for
   transfers performed by the edge, every root whose obligation region is in
   that set MUST be definitely `Consumed`; both `Owned` and `MaybeConsumed`
   fail this requirement. Roots belonging to regions outside the set retain
   their path-sensitive state.
5. The exited-region set is determined structurally:
   1. normal block or arm completion exits that block or arm region,
   2. `yield` exits its innermost block-expression region; when that region is
      the function/method body, completion also exits the callable,
   3. `?` exits every active region through the innermost enclosing `result`
      body, but not a lexically enclosing region that remains after the result
      expression produces its `Err` or `None` value,
   4. a checked numeric fault exits every active region through the innermost
      enclosing `checked` body, but not a lexically enclosing region that
      remains after the checked expression produces `Err(NumericError)`,
   5. `break` and `continue` exit every nested region up to the selected loop
      edge,
   6. `return` and ordinary function/method completion exit every active region
      in that callable.
6. If completion of an inner expression immediately causes another enclosing
   exit, each edge is checked in order. For example, `?` may first exit a
   nested `result` region and its produced `Err` may then become the enclosing
   function result; the latter edge also checks the function region.
7. An outer-region root MAY remain `Owned` across an inner `yield`, `?`, checked
   fault, normal completion, `break`, or `continue` when that edge does not exit
   the root's obligation region. Consuming such a root inside the nested region
   still changes its path-sensitive state in the enclosing region.
8. `yield move x` or an equivalent fresh ownership-bearing result MAY transfer
   a root across the exited region when the block result type permits that root
   shape. Yielding an ownership-free value while an exited-region root is
   `Owned` or `MaybeConsumed` MUST fail with `OWNERSHIP_PATH_UNBALANCED`.
9. A direct root not discharged or transferred on a required region exit MUST
   fail with `OWNERSHIP_PATH_UNBALANCED`.
10. A conditional root that is not definitely consumed or transferred on a
    required region exit MUST fail with `OWNERSHIP_PATH_UNBALANCED`, regardless
    of its runtime variant.
11. A path with no normal continuation creates no typed control-flow edge that
    exits an obligation region and therefore has no discharge requirement on
    that path. This includes fatal runtime termination and any evaluation
    carrying the no-normal-continuation fact from `type-system.md`. The
    expression and all syntactically present source remain subject to ordinary
    ownership validation, and a structurally uninhabited type other than exact
    `Never` does not imply this control fact.
12. The discard target `_`, an unused expression statement, and overwrite
    assignment MUST NOT discharge ownership-root values.
13. Attempting to discard an ownership-bearing value through any Rule 12 form,
    including `_ = move x`, MUST fail with `OWNERSHIP_DISCARD_FORBIDDEN`.
14. `drop_own move x` accepts any non-resource ownership-root type, consumes the
    complete root value (including a present conditional payload), and returns
    `Unit`.
15. `drop_own x` is parse-valid only to diagnose the missing `move` marker and
    MUST fail with `DROP_OWN_REQUIRES_MOVE`.
16. Code that needs to preserve ordinary `Err`/`None` control flow SHOULD use
    `(move r)?` or `match move r`; `drop_own move r` intentionally discards the
    complete carrier value.
17. A resource root MUST instead be transferred, closed through its consuming
    bang terminal, or managed by a standard-library scoped resource API.
18. Region-exit checks are keyed by binding identity rather than source
    spelling. Every hidden outer root and same-spelled inner root is checked as
    a separate obligation on each applicable edge.

### Path-sensitive Ownership Analysis

Rules:

1. Ownership-root flow analysis MUST be path-sensitive.
2. Each root and each source consumed by `own` MUST be tracked with abstract state:
   1. `Owned` (definitely available),
   2. `Consumed` (definitely unavailable),
   3. `MaybeConsumed` (incoming paths disagree).
3. Operations requiring an available root MUST reject `Consumed` and `MaybeConsumed` states.
4. A successful `move`, `drop_own move`, `(move r)?`, `match move r`, or consuming resource terminal changes the source root state to `Consumed` on that path.
5. `own x` changes source binding `x` to `Consumed` and creates a fresh direct root in the destination expression flow.
6. At a control-flow join:
   1. all `Owned` inputs merge to `Owned`,
   2. all `Consumed` inputs merge to `Consumed`,
   3. every mixed merge produces `MaybeConsumed`.
7. Loop analysis MUST use deterministic fixed-point iteration; inability to prove `Owned` produces `MaybeConsumed`.
8. A second consuming operation on a `Consumed` root MUST fail with `OWNERSHIP_DOUBLE_MOVE`.
9. A borrow or other non-consuming use of a `Consumed` or `MaybeConsumed` binding MUST fail with `OWNERSHIP_USE_AFTER_DISCHARGE`.
10. A consuming operation on `MaybeConsumed` MUST fail with `OWNERSHIP_DOUBLE_MOVE`.
11. A reference value derived from one or more roots carries their path-sensitive provenance through local copies, rebindings, and merges; use requires every root in the current provenance set to be `Owned` and otherwise follows Rule 9.
12. Reference provenance is function-local and MUST NOT weaken the general non-escaping reference rules.
13. Each root state retains its lexical obligation-region identity until that
    root is consumed. A destination binding created by transfer receives its
    own region identity under Exact-once Discharge Rule 3.
14. Every exited-region imbalance, including end-of-function imbalance,
    remains `OWNERSHIP_PATH_UNBALANCED`.
15. Implementations MAY use interprocedural summaries only when they preserve these rules.

### Ownership Diagnostic Reporting Policy

1. This policy is an explicit subsystem reporting boundary under
   `../errors/ordering.md`: implementations MUST emit exactly one primary
   ownership error per failing function analysis instance.
2. When an analysis instance has multiple candidate ownership roots, the primary
   error is the candidate with the earliest canonical ordering key under
   `../errors/ordering.md`. This code-specific selection occurs before general
   retained-diagnostic ordering and is not affected by ownership-analysis
   traversal order. Further ownership errors in that function are not separately
   emitted, even when they would otherwise be independently actionable.
3. Additional ownership-cause locations SHOULD be attached as `related` spans or
   notes when distinct locations are available. An independent suppressed error
   is not converted into a generic related span.
4. Primary spans, and related spans when emitted, MUST remain deterministic
   under `../errors/ordering.md`.

### Ownership Profile Status

1. Conforming v0.1 baseline implementations MUST enable the ownership surface.
2. Implementations MAY expose a non-baseline ownership-disabled mode.
3. In ownership-disabled mode, `Own[...]`, `own`, `move`, and `drop_own` MUST fail with `OWNERSHIP_PROFILE_DISABLED`.
4. `Box[T]` is ordinary value/layout surface and remains available when the ownership profile is disabled.
5. v0.1 defines no capability tokens; future effect authorization MUST remain distinct from ownership discharge.
