# Compile-time vs Runtime Validation Matrix

Scope: Normative for v0.1 unless this file or a section states otherwise.

This section fixes channel selection for validation failures in v0.1.

### Channel Rules

1. If a violation is statically decidable, compilation MUST fail with `compile_error`.
   1. Exception in v0.1 baseline: `DETERMINISM_UNAVAILABLE` is fixed to runtime value channel and MUST NOT be emitted as `compile_error`.
2. If violation outcome depends on runtime values, execution MUST produce `value: Err(...)` for fallible APIs.
3. Unrecoverable runtime termination (for example explicit abort paths) uses `runtime_error`.
4. Same failure cause MUST use the same diagnostic code across channels.
5. In v0.1 baseline, well-typed non-bang code MUST NOT terminate via `runtime_error`; explicit runtime-fatal operations are bang surfaces and are rejected in non-bang contexts at compile time.


### Const-evaluable Domain (v0.1)

`const-evaluable` means the implementation can determine the relevant predicate without runtime execution.

Minimum required support:

1. scalar literals, the Unit literal `()`, and pure scalar operators over
   const-evaluable operands,
2. static shape/rank/dtype/axis-name metadata derived from tensor constructors and pure metadata-preserving transforms,
3. literal index specs and literal `.with_sizes(...)` key/value bindings, including key-membership and domain-scalar checks,
4. contextual fixed-width integer literal representability checks,
5. contextual `Nat`/`PositiveNat` integer literal domain checks and explicit domain scalar conversions whose input is const-evaluable,
6. constructor-known tensor constant classes (`zeros`, `ones`, `nan`) when failure predicate depends only on those constructor guarantees.

`const-evaluable` in v0.1 does not require full tensor element-value execution or backend-dependent kernel simulation.

### Const-evaluable Operator Table (Minimum Required)

This table defines required v0.1 const-evaluable classification at operator/form level.

| Form / Operator | v0.1 requirement | Notes |
|---|---|---|
| Scalar literals (integer, floating, `Bool`, `Tri`, `Str`) and Unit literal `()` | MUST be const-evaluable | literal-only |
| Prior `const` references | MUST be const-evaluable | subject to forward-reference/cycle rules |
| Unary scalar operators (`!`, unary `-`) | MUST be const-evaluable when operand is const-evaluable | type-valid scalar forms only |
| Binary scalar arithmetic (`+`, `-`, `*`, `/`, `/.`, `%`) | MUST be const-evaluable when both operands are const-evaluable | runtime numeric fault codes may surface at compile-time |
| Scalar comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) | MUST be const-evaluable when both operands are const-evaluable | requires normal type legality |
| Scalar logical (`&&`, `||`) | MUST be const-evaluable when both operands are const-evaluable | short-circuit semantics preserved |
| Domain scalar conversions (`Nat.from_i64`, `PositiveNat.from_i64`) | MUST be const-evaluable when the input is const-evaluable | only the domain predicate is required |
| Tensor constructors (`zeros`, `ones`, `nan`) with literal/static metadata args | MUST be const-evaluable for metadata-dependent predicates | does not require full element execution |
| Cast forms (`astype`, `wrapping_cast`, `bitcast`) | MUST be const-evaluable when source metadata and dtype predicates are const-evaluable | channel/precedence rules below apply |
| Function or method calls (general case) | MUST NOT be required as const-evaluable in v0.1 | implementations MAY extend, but conformance MUST NOT require it |
| Bang calls (`*!`) | MUST NOT be const-evaluable | effectful by definition |

Boundary constraints:

1. Interprocedural propagation across function boundaries is not part of required const-evaluable analysis in v0.1.
2. Backend/device capability assumptions are not const-evaluable unless explicitly fixed by source-level context.


### Deterministic Validation Stage Order

When multiple failures are possible, implementations MUST select primary failure by this precedence chain:

1. schema/axis/shape/index/dim-solver validation,
2. dtype/device compatibility,
3. determinism-profile availability,
4. numeric primitive execution.

This order applies regardless of lowering/fusion choices.


### Per-family Matrix

#### Schema and Type Layer

1. Grammar/type/signature violations are compile-time only.
2. Examples: `CONTRACT_NON_ATOMIC_AXIS`, `AXIS_PACK_ANON_DUPLICATE`,
   `AXIS_PACK_UNBOUND`, `AXIS_MAPPING_UNSUPPORTED`,
   `PURE_COMBINATOR_EFFECTFUL_CALLBACK`, parser/lexer/type-check codes.
3. Local-binding mutability, assignment-target classification, and top-level const classification are compile-time only:
   1. `IMMUTABLE_BINDING_ASSIGN` (local immutable binding or immutable struct field assignment),
   2. `ASSIGN_TARGET_NOT_ASSIGNABLE` (assignment target is not a storage target),
      `ASSIGNMENT_TYPE_MISMATCH` (typed initializer or writable assignment RHS
      is not assignable to its expected type),
   3. `VAR_DECL_FORBIDDEN`,
      1. detection phase MAY vary (`parse` or later static phase) but code MUST remain `VAR_DECL_FORBIDDEN`,
   4. `STRUCT_FIELD_DEFAULT_FORBIDDEN`,
   5. `CONST_INIT_NOT_CONST`,
   6. `CONST_FORWARD_REFERENCE`,
   7. `CONST_CYCLE`,
   8. `STRUCT_INIT_UNKNOWN_FIELD`,
   9. `STRUCT_INIT_DUPLICATE_FIELD`,
   10. `STRUCT_INIT_MISSING_FIELD`,
   11. `OVERLOAD_MARKER_REQUIRED`,
   12. `OVERLOAD_VISIBILITY_MISMATCH`,
   13. `OVERLOAD_RETURN_ONLY_DIFFERENCE`,
   14. `OVERLOAD_OVERLAP`,
   15. `OVERLOAD_AMBIGUOUS`,
   16. `NON_BANG_MUT_PARAM_FORBIDDEN`,
   17. `REF_ESCAPE_FORBIDDEN`,
   18. `METHOD_RECEIVER_INVALID`,
   19. `SELF_TYPE_INVALID`,
   20. `RANGE_EXPR_UNSUPPORTED`,
   21. `RETURN_OUTSIDE_FUNCTION`,
   22. `RETURN_VALUE_REQUIRED`,
   23. `RETURN_VALUE_FORBIDDEN`,
   24. `RETURN_TYPE_MISMATCH`,
   25. `OWNERSHIP_PATH_UNBALANCED` (when ownership surface is enabled),
   26. `OWNERSHIP_TYPE_POSITION_FORBIDDEN` (when `Own[...]` appears outside a permitted root shape),
   27. `REFERENCE_TYPE_POSITION_FORBIDDEN` (when an ordinary generic argument or associated type binding conceals a reference),
   28. `OWNERSHIP_TRANSFER_REQUIRES_MOVE` (when a named root is transferred without `move`),
   29. `OWNERSHIP_DISCARD_FORBIDDEN` (when an ownership-bearing value is discarded or overwritten),
   30. `MOVE_REQUIRES_OWN` (when ownership surface is enabled),
   31. `OWN_REQUIRES_NON_OWN` (when ownership surface is enabled),
   32. `OWN_REQUIRES_LOCAL_BINDING` (when `own` does not resolve to a consumable local/parameter),
   33. `DROP_OWN_REQUIRES_MOVE` (when ownership surface is enabled),
   34. `RESOURCE_DROP_FORBIDDEN` (when `drop_own move` targets a designated resource root),
   35. `OWNERSHIP_DOUBLE_MOVE` (when ownership surface is enabled),
   36. `OWNERSHIP_USE_AFTER_DISCHARGE` (when ownership surface is enabled),
   37. `OWNERSHIP_GUARD_CONSUME_FORBIDDEN` (when a tentative match-guard binding is consumed),
   38. `OWNERSHIP_PROFILE_DISABLED` (when ownership surface is disabled and any ownership surface usage is present, including `Own[...]` type usage),
   39. `TYPE_RECURSIVE_LAYOUT_REQUIRES_INDIRECTION`,
   40. `STRUCT_INIT_PROPERTY_FORBIDDEN`,
   41. `PROP_ACCESSOR_MISSING`,
   42. `PROP_ACCESSOR_SIGNATURE_MISMATCH`,
   43. `PROPERTY_ASSIGN_FORBIDDEN`,
   44. `CALL_POSITIONAL_AFTER_NAMED`,
   45. `CALL_DUPLICATE_NAMED_ARG`,
   46. `CALL_UNKNOWN_NAMED_ARG`,
   47. `FSTRING_UNRESOLVED_NAME`,
   48. `FSTRING_INVALID_PLACEHOLDER`,
   49. `UNSAFE_REQUIRED`,
   50. `EXTERN_VISIBILITY_FORBIDDEN`,
   51. `EXTERN_SIGNATURE_UNSUPPORTED`,
   52. `EXTERN_ABI_UNSUPPORTED`,
   53. `RAW_POINTER_TYPE_UNSUPPORTED`,
   54. `RAW_POINTER_USE_UNSUPPORTED`,
   55. `DYN_TRAIT_UNSUPPORTED`,
   56. `UNION_TYPE_ARITY_MISMATCH`,
   57. `UNION_TYPE_UNSUPPORTED`,
   58. `TYPE_ARG_ARITY_MISMATCH`,
   59. `TRAIT_BOUND_INVALID`,
   60. `TRAIT_BOUND_NOT_SATISFIED`,
   61. `TRAIT_IMPL_ASSOC_TYPE_MISMATCH`,
   62. `ASSOCIATED_TYPE_PROJECTION_INVALID`,
   63. `ASSOCIATED_TYPE_CYCLE`,
   64. `CLOSED_TRAIT_IMPL_FORBIDDEN`,
   65. `TRAIT_IMPL_SIGNATURE_MISMATCH`,
   66. `VISIBILITY_VIOLATION` (including inaccessible items or members, external construction, and an externally reachable type contract whose alias-expanded structure exposes a private declaration or a foreign package declaration without the required contract-local public re-export),
   67. `LOCAL_BINDING_REDECLARATION`,
   68. `NAME_NOT_FOUND`,
   69. `RESERVED_BUILTIN_NAME` (including attempts to introduce contextual type spelling `Self`),
   70. `MUTABLE_CAPTURE_FORBIDDEN` (when a closure captures a mutable binding or writes storage rooted in a captured binding),
   71. `PROGRAM_ENTRY_MISSING`,
   72. `PROGRAM_ENTRY_DUPLICATE`,
   73. `PROGRAM_ENTRY_SIGNATURE_INVALID`,
   74. `REFERENCE_TARGET_INVALID`,
   75. `PATTERN_TYPE_MISMATCH`,
   76. `PATTERN_PAYLOAD_FORM_MISMATCH`,
   77. `PATTERN_ARITY_MISMATCH`,
   78. `PATTERN_DUPLICATE_FIELD`,
   79. `PATTERN_UNKNOWN_FIELD`,
   80. `PATTERN_MISSING_FIELD`,
   81. `PATTERN_DUPLICATE_BINDING`,
   82. `PATTERN_OWNERSHIP_BINDING_OVERLAP`.
4. Property declaration/accessor validation is compile-time only:
   1. missing/wrong property accessor contracts use `PROP_ACCESSOR_MISSING` / `PROP_ACCESSOR_SIGNATURE_MISMATCH`,
   2. these declaration-level codes take primary precedence over ordinary receiver/effect-signature validation for the same accessor declaration.
5. Public-API type closure is compile-time declaration validation:
   1. private source aliases with completely public expansions are accepted,
   2. a private nominal type or trait reached after expansion uses `VISIBILITY_VIOLATION`,
   3. in package mode, a foreign nominal type or trait without an exact canonical-target `pub import` in the contract-owning module also uses `VISIBILITY_VIOLATION`,
   4. public-trait/public-target conformance generic bounds and headers are included after impl authorization and coherence succeed,
   5. name, alias, projection, ordinary type-position, and declaration-specific structural checks retain their existing precedence, and
   6. visibility closure then precedes body, initializer, and downstream-use validation.
6. Program-entry validation is compile-time only and runs only for executable
   invocations after invocation/package validation and ordinary source
   compilation have succeeded. Top-level `main!` candidates in the entry
   source defer their own duplicate/overload failure to
   `PROGRAM_ENTRY_DUPLICATE` under `program-model.md`.
7. Future `CAPABILITY_*` diagnostics are extension-profile only in v0.1 and are not part of required baseline conformance unless that extension profile is explicitly enabled.

#### Baseline Broadcasting and Directed Expansion

1. `BROADCAST_SHAPE_MISMATCH` applies to common-shape broadcasting and directed expansion under `tensor-model.md`.
2. The code is emitted through `compile_error` when all relevant operand or target shapes are const-evaluable.
3. Otherwise, the consuming operation returns runtime `Err(ShapeError)` with the same code.

#### Empty Reduction Domains

1. Primitive reduction-form `max` MUST evaluate
   `REDUCTION_EMPTY_DOMAIN` after axis resolution and normalized selected-axis
   ordering, but before dtype validation or output traversal.
2. The failure predicate is that the selected-axis extent product is `0`. The
   product of an empty selected-axis set is `1`.
3. A const-evaluable failing shape MUST use `REDUCTION_EMPTY_DOMAIN` in the
   `compile_error` channel.
4. A runtime-dependent failing shape MUST return `Err(ShapeError)` with the same
   code.
5. The required `axis` diagnostic datum is the lowest `AxisIndex` among selected
   zero-length axes, independent of selector spelling order.
6. A zero-length retained axis MUST NOT suppress the failure. Conversely, a
   zero-length retained axis with nonzero selected-axis extent is a successful
   empty output.

#### Transform and Dim-solver (`view`/`rearrange`/`inflate`/`reduce`/`contract`/`einop`)

Unless a family-specific rule below states otherwise, ordinary name, call,
callable-type, and effect validation owned by the language core precedes
tensor-specific transform validation. For `map_axis`, `grad`, and
`value_and_grad`, an effectful callback supplied to `callback[pure]` therefore
selects `PURE_COMBINATOR_EFFECTFUL_CALLBACK` before a tensor-specific transform
argument failure in the same construction. Ordered-phase `reduce_by` is the
closed exception: its axis-partition checks in Rule 5 precede callback-chain
type and effect validation.

1. `WITH_SIZES_UNKNOWN_KEY`:
   1. compile-time when the offending key is const-evaluable,
   2. otherwise schema invocation returns runtime `Err(...)` through the operation's existing error family with the same code,
   3. key-membership validation precedes all dim-solver outcomes.
2. `AMBIGUOUS_DIMS`, `INCONSISTENT_DIMS`, `DIM_BINDING_REQUIRED`, `NUMEL_MISMATCH_GROW`, `NUMEL_MISMATCH_SHRINK`, `SEGMENT_STRADDLE`, `MULTI_INPUT_NOT_ALLOWED`, `CONTRACT_REPEATED_LABEL_DIM_MISMATCH`:
   1. compile-time if const-evaluable,
   2. otherwise runtime `Err(...)` with same code.
3. `AXIS_PACK_ANON_DUPLICATE`, `AXIS_PACK_UNBOUND`,
   `AXIS_MAPPING_UNSUPPORTED`, `CONTRACT_OUTPUT_LABEL_DUPLICATE`,
   `CONTRACT_OUTPUT_LABEL_UNBOUND`, and `EINOP_OUTPUT_LABEL_DUPLICATE` are
   compile-time only (shape-independent schema validation).
   `AXIS_MAPPING_UNSUPPORTED` precedes all dimension, segment, numel, dtype,
   and affine-map diagnostics for the same invocation.
4. `REARRANGE_OUTPUT_DTYPE_MISMATCH` is compile-time only because input element
   types and schema segment provenance are static. Statically decidable
   rearrange schema, axis, dimension, segment, and numel failures precede this
   dtype relation; outputs are then checked in right-hand-side order. A
   runtime-dependent shape or size failure remains an execution-time check and
   does not suppress a compile-time dtype failure.
5. After receiver identity resolution, a retained contextual `reduce_by`
   candidate with no valid argument interpretation fails at compile time with
   `REDUCE_BY_ARGUMENTS_INVALID`: canonical `ReduceSchema` requires
   `ReduceByArgs`, while every other receiver requires ordinary `ArgList`
   syntax. Neither candidate falls back across that identity boundary.
6. Ordered-phase `reduce_by` axis-partition validation is compile-time only:
   1. `REDUCE_BY_PHASE_NON_REDUCED_AXIS`,
   2. `REDUCE_BY_PHASE_OVERLAP`,
   3. `REDUCE_BY_PHASE_INCOMPLETE`.
7. A valid `reduce_by` phase partition is typed left-to-right after invocation
   carrier inference:
   1. phase `0` receives `TensorView[C::Elem]`,
   2. each phase's successful payload type becomes the next phase's expected
      `TensorView` element type,
   3. each successful payload must inhabit the documented `DType[T]` family,
   4. each phase contributes its specialized callback result's exact normalized
      error argument, without contextual widening,
   5. the first incompatible phase callable or unsupported payload type in
      source order fails with `CALL_ARG_TYPE_MISMATCH`,
   6. axis-partition diagnostics in Rule 6 take precedence over callback-chain
      type or effect diagnostics for the same schema invocation, including
      `PURE_COMBINATOR_EFFECTFUL_CALLBACK`.
8. For `ReduceBySchema` callback inference:
   1. a complete same-expression invocation fixes the concrete carrier and
      `T_0` before input-dependent inline callbacks are checked,
   2. direct invocation and canonical `then`/unary-tuple `apply` desugarings use
      the same typing order,
   3. without an invocation, an explicit first-phase closure parameter or a
      fully specialized concrete first-phase callable may fix `T_0` at the
      construction site,
   4. callback result and error variables are inferred only by ordinary forward
      body typing,
   5. a stored schema initializer that remains underdetermined fails immediately
      with `BINDING_TYPE_UNDETERMINED`; a later invocation cannot repair it,
   6. a stored schema whose callback chain is fully determined at construction
      is an ordinary monomorphic value,
   7. Rule 6 axis-partition diagnostics precede callback-chain typing; after
      those checks, any callback-family, arity, result-shape, or effect violation
      decidable without the missing `T_0` precedes
      `BINDING_TYPE_UNDETERMINED`.

#### Indexing (`slice` / `get_at` / `set_at` / `set_at!`)

1. `INDEX_SPEC_REST_DUPLICATE`:
   1. applies before source-rank arity validation when more than one `Rest` is present,
   2. compile-time when the complete range/index vector is const-evaluable,
   3. otherwise runtime `Err(IndexError)` with the same code.
2. `INDEX_SPEC_ARITY_MISMATCH`:
   1. applies after duplicate-`Rest` validation when consuming-item count exceeds source rank,
   2. compile-time when the vector and source rank are const-evaluable,
   3. otherwise runtime `Err(IndexError)` with the same code.
3. `BROADCAST_SHAPE_MISMATCH`:
   1. applies to incompatible `Take` common-shape broadcasting and scatter-value directed expansion,
   2. compile-time when all relevant shapes are const-evaluable,
   3. otherwise runtime `Err(ShapeError)` with the same code.
4. `INDEX_OUT_OF_BOUNDS`:
   1. applies to `At` and `Take` coordinates after signed normalization; `Slice` bounds clip and do not use this code,
   2. compile-time if index coordinates and axis lengths are const-evaluable,
   3. otherwise runtime `Err(IndexError)` with same code.
5. `SLICE_STEP_ZERO`:
   1. compile-time if the zero step is const-evaluable,
   2. otherwise runtime `Err(IndexError)` with same code.
6. `SCATTER_SET_DUPLICATE_INDEX`:
   1. compile-time when duplicate targets are statically provable,
   2. otherwise runtime `Err(IndexError)` after runtime duplicate check.

#### `map_axis`

1. `MAP_AXIS_IN_AXES_ARITY_MISMATCH`, `MAP_AXIS_NO_MAPPED_ARGS`, `MAP_AXIS_NON_TENSOR_MAPPED` are compile-time when call shape/signature is statically known.
2. `MAP_AXIS_BATCH_SIZE_MISMATCH`, `MAP_AXIS_OUT_AXIS_OUT_OF_RANGE`, `MAP_AXIS_RESULT_SHAPE_MISMATCH`, `MAP_AXIS_RESULT_OVERLAY_MISMATCH`, `MAP_AXIS_OUT_NAME_DUPLICATE`, `MAP_AXIS_EMPTY_BATCH_UNSUPPORTED`:
   1. compile-time if const-evaluable,
   2. otherwise runtime `Err(...)` with same code.
3. For each later successful result, shape mismatch validation precedes overlay mismatch validation; both select the earliest mismatching 0-based result index under left-to-right semantics.
4. `MAP_AXIS_RES_FAIL` remains a runtime fail-fast event.
5. `MAP_AXIS_OPT_NONE` is a runtime short-circuit note path when emitted (optional emit-if-present in baseline v0.1).

#### `grad` / `value_and_grad`

1. `GRAD_ARGNUM_OUT_OF_RANGE`, `GRAD_NON_TENSOR_ARG`, `GRAD_NON_FLOAT_DTYPE`, `GRAD_PRIMAL_NOT_SCALAR`:
   1. compile-time if const-evaluable,
   2. otherwise runtime `Err(...)` with same code.
2. `GRAD_LOWERING_FAILED`, `GRAD_UNSUPPORTED_PRIMITIVE`:
   1. compile-time when proven before execution,
   2. otherwise runtime `Err(...)` with same code.
3. Failure selection follows the canonical AD-support pipeline:
   1. inability to form the pure Core Tensor IR graph is
      `GRAD_LOWERING_FAILED`,
   2. rejection by the closed source-active or passive-auxiliary table after
      successful lowering is `GRAD_UNSUPPORTED_PRIMITIVE`.

#### Unsafe / FFI

1. `UNSAFE_REQUIRED`, `EXTERN_VISIBILITY_FORBIDDEN`, `EXTERN_SIGNATURE_UNSUPPORTED`, `EXTERN_ABI_UNSUPPORTED`, `RAW_POINTER_TYPE_UNSUPPORTED`, `RAW_POINTER_USE_UNSUPPORTED`, and `DYN_TRAIT_UNSUPPORTED` are compile-time only.
2. `EXTERN_LINKAGE_FAILED`:
   1. MUST be emitted in `compile_error` when linkage failure is statically decidable,
   2. MUST be emitted in `runtime_error` when linkage failure is not statically decidable.

#### Determinism Availability

1. `DETERMINISM_UNAVAILABLE`:
   1. applies to an explicit strict request through a profile-bearing API,
   2. in v0.1 baseline, MUST be raised through fallible runtime value channel (`Res::Err` plus diagnostics),
   3. in v0.1 baseline, MUST NOT be emitted as `compile_error`,
   4. no silent fallback to `default` is permitted when strict profile is requested.
2. Failure to establish a profile-less active strict context is an
   implementation activation/certification failure outside source execution;
   it MUST NOT be projected into an API's undeclared error union.

#### Domain Scalar Conversion

1. `DOMAIN_INVALID_VALUE`:
   1. compile-time if a contextually typed `Nat`/`PositiveNat` integer literal or explicit domain conversion input is const-evaluable,
   2. otherwise runtime `Err(DomainError)` with the same code.
2. Domain scalar failures are not cast failures and MUST NOT use `CAST_INVALID_VALUE` or `CAST_INVALID_DTYPE_PAIR`.
3. Domain scalar conversion failures MUST NOT use unrecoverable runtime trap/panic channel in v0.1.

#### Numeric Faults

1. `INT_OVERFLOW`, `INT_DIV_BY_ZERO`, `INT_REM_BY_ZERO`, `CAST_INVALID_VALUE`, `CAST_INVALID_DTYPE_PAIR`:
   1. compile-time if failing expression is const-evaluable under v0.1 rules,
   2. otherwise runtime `Err(...)` with same code.
   3. for scalar integer operators, the runtime `Err(NumericError)` path exists only inside `checked {}`; a potentially faulting operator outside that boundary is rejected with `NUMERIC_CHECKED_CONTEXT_REQUIRED` before execution,
   4. tensor and cast APIs use their declared `Res` return channels and do not require a scalar `checked {}` boundary.
   5. fixed-width integer literal representability is always statically
      decidable after contextual typing or defaulting and therefore uses
      `INT_OVERFLOW` in the compile channel.
2. `/.` follows floating/complex semantics and therefore does not emit integer-divide fault codes.
3. `const` initializer precedence for cast-related failures is fixed:
   1. if initializer is not const-evaluable, primary compile error MUST be `CONST_INIT_NOT_CONST`,
   2. when `CONST_INIT_NOT_CONST` is emitted, implementations MUST NOT additionally report `CAST_INVALID_DTYPE_PAIR` or `CAST_INVALID_VALUE` for that initializer as primary error,
   3. only const-evaluable initializers may emit cast-failure primary errors.
4. Cast runtime channel is fixed for fallible cast APIs (`astype`, `wrapping_cast`, `bitcast`):
   1. non-const-evaluable cast failures MUST return runtime `Err(...)` with cast cause code identity preserved,
   2. cast failures MUST NOT use unrecoverable runtime trap/panic channel in v0.1.
5. `wrapping_cast` and `bitcast` are const-eligible:
   1. if arguments and dtype predicates are const-evaluable, legality checks MUST run at compile-time,
   2. successful const-evaluable forms MAY be constant-folded.
