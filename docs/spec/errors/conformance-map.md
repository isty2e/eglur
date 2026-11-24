# Mapping to Conformance Expectations

Scope: Normative for v0.1 unless this file or a section states otherwise.

The conformance suite rooted at `../../../tests/conformance/manifest.yaml` assumes:

1. `compile_error.code` matches exactly for compile-channel codes in `compile-codes.md` and `extension-codes.md`, and MUST also accept matching cause codes from `runtime-codes.md` when statically decidable per `../language/validation-matrix.md` (`code-channels.md`), except causes explicitly fixed to value channel in v0.1 baseline.
   1. `DETERMINISM_UNAVAILABLE` is fixed to value channel in v0.1 baseline.
2. `diagnostic-contracts.yaml` is the field-level authority used by required
   suites and release tooling.
   1. Its global `SourceSpan`, `PrimarySpan`, and `RelatedSpan` schemas and
      normative examples MUST pass the contract loader's exact-shape,
      coordinate, range, and ordering checks.
   2. This static validation does not make `related` emission mandatory.
3. Primary diagnostic objects MUST explicitly include `message_contains` and
   `data_keys`; either list MAY be empty.
   1. `message_contains` contains the contract's portable all-of literals plus
      one concrete rendering for each message template.
   2. `message_contains_any` is required exactly when the contract defines
      one-of groups.
   3. Required suites MUST NOT assert contextual message prose that cannot be
      represented portably.
   4. `data_equals` is optional.
   5. If `data_equals` is present:
      1. each key MUST also appear in `data_keys`,
      2. each expected value MUST be a `Str`, signed `i64`, or `Bool` YAML
         scalar,
      3. the machine contract MUST list the key in `exact_value_keys`,
      4. emitted diagnostic `data` MUST match by exact scalar kind/value.
   6. `data_keys` MUST equal unconditional and active conditional keys in the
      machine contract; required suites MUST NOT force emit-if-present keys.
   7. If the current required-key set contains a key that selects a
      `conditional_required` branch, a required fixture MUST fix that selector
      in `data_equals`; otherwise the fixture has not established which
      conditional shape it asserts.
   8. These requirements apply to `compile_error`, `runtime_error`, and
      structured `value_diagnostics` entries.
   9. Required type-system mismatch fixtures MUST preserve the semantic data
      partitions in `data-keys.md`: directional type pairs, source-ordered or
      symmetric `left_type`/`right_type` pairs, normalized signature pairs,
      count/form fields, and trait obligations. Representative fixtures MUST
      use `data_equals` to lock canonical type, signature, and count rendering.
4. Required suites MUST NOT use `help_contains`, `related_contains`, or
   top-level `diagnostic_contains`. Optional suites MAY use them as
   non-blocking rendering probes.
   1. A `compile_error` assertion matches the ordered primary diagnostic under
      `ordering.md`; it does not assert that the compiler emitted exactly one
      error.
   2. Required source suites do not assert recovery breadth, a complete retained
      diagnostic set, or a diagnostic limit.
5. In baseline v0.1 coverage policy:
   1. `error`-severity codes are `hard_required_asserted` unless explicitly marked conditional/excluded below,
   2. `note`/`warning`-severity codes are `optional_emit_if_present`:
      1. conformance does not require emission,
      2. if emitted, code/token/data-key contracts remain mandatory.
6. For fallible `Res`/`Opt` APIs, non-statically-decidable failure conformance is validated through primary `value` outcomes (`Err`/`None`) plus optional secondary diagnostics, not through `runtime_error`.
7. For statically decidable fallible failures, conformance MUST use `compile_error` with the same cause code (channel differs, cause code does not), except causes explicitly fixed to value channel in v0.1 baseline (`DETERMINISM_UNAVAILABLE`).
8. Short-circuit failures in sequence combinators include `index <N>` with 0-based indexing:
   1. required error paths: traverse/fold families (`TRAVERSE_RES_FAIL`, `FOLD_RES_FAIL`),
   2. optional emit-if-present paths: `TRAVERSE_OPT_NONE` diagnostics.
9. `map_axis` coverage requires core callback-effect validation to precede
   tensor-specific transform-argument validation in one multi-fault
   construction. Its short-circuit note path `MAP_AXIS_OPT_NONE` is optional
   emit-if-present in baseline; if emitted, it MUST include the required index
   token/data contract.
10. Required baseline suites MUST NOT assert mandatory emission for `optional_emit_if_present` note/warning codes.
   1. Emission-contract assertions for these codes belong in optional probe suites (`required: false`).
11. Axis overlay suites require:
   1. required message projections for the corresponding machine contracts,
   2. partial `with_names` patch coverage for omitted-name preservation and empty-map no-op behavior,
   3. simultaneous-patch coverage that accepts valid name swaps,
   4. final-overlay duplicate coverage against retained names with deterministic lowest-axis duplicate selection,
   5. bounds-first precedence when one patch can also produce a duplicate name,
   6. failed `with_names!` coverage that leaves the receiver overlay unchanged,
   7. `MAP_AXIS_RESULT_OVERLAY_MISMATCH` coverage that asserts:
      1. exact binding equality against the first successful result overlay without treating `Map` insertion order as semantic,
      2. the earliest mismatching result index and both overlay summaries,
      3. shape-mismatch precedence when one result differs in both shape and overlay;
   8. representative both-fail tensor paths (axis mismatch + shape mismatch both possible) that assert axis-first primary failure;
   9. primitive `reshape` coverage that:
      1. preserves the complete overlay for exact shape identity,
      2. drops the complete overlay for merge, split, singleton insertion/removal, and rank-preserving shape changes,
      3. accepts direct movement chaining over `TensorView` without implicit materialization,
      4. returns `NOT_A_VIEW` rather than copying for a shape-valid non-affine
         `TensorView` reshape,
      5. gives `ShapeError` precedence over affine representability when both
         checks could fail;
   10. primitive `expand` coverage that:
      1. shifts source overlay indices right when lower-rank input is promoted,
      2. leaves prepended leading axes unnamed,
      3. preserves names on aligned source axes whose size expands from `1`, including expansion to `0`.
12. `AXIS_ALIASED_HEADER` is non-baseline in v0.1 and excluded from hard-required baseline coverage.
13. Grad/scatter suites require baseline `GRAD_*` and
    `SCATTER_SET_DUPLICATE_INDEX` tokens when those failure paths are exercised.
    Grad coverage requires core callback-effect validation to precede
    tensor-specific `argnum` validation in one multi-fault construction. It MUST
    distinguish source-active from passive operands and reverse-only operations:
    passive unsupported primitives do not poison an otherwise valid transform,
    duplicate passive gather indices accumulate into the base cotangent, and
    source-active operations omitted from the closed table fail with
    `GRAD_UNSUPPORTED_PRIMITIVE`. The suites also require a differentiated
    `TensorView` parameter to remain `TensorView` in the returned callable while
    producing an owning gradient over the view's logical shape.
    `GRAD_CUSTOM_KERNEL_NO_VJP` and `GRAD_RESULT_SHAPE_MISMATCH` are reserved
    non-baseline codes until a typed custom-operation and derivative-registration
    surface is standardized.
14. Contract semantic-failure suites require:
   1. `CONTRACT_OUTPUT_LABEL_UNBOUND` as compile-channel coverage for an output label absent from all input labels, including bound-before-duplicate precedence;
   2. `CONTRACT_OUTPUT_LABEL_DUPLICATE` as compile-channel coverage for the second occurrence of a bound output label, including left-to-right precedence over a later unbound label;
   3. `CONTRACT_REPEATED_LABEL_DIM_MISMATCH` as value-channel coverage when repeated-label dimension mismatch crosses a non-const-evaluable boundary, including size-1 broadcast candidates and cross-operand occurrences;
   4. nonuniform repeated-label values that distinguish diagonal selection from
      full reduction, including retained, batched, interleaved, generalized
      multi-occurrence, multiple-group remapping, and zero-extent cases;
   5. strict-name contract coverage for:
      1. accepted `with_aliases` external-name-to-canonical-label mapping,
      2. invalid alias targets with `AXIS_MISMATCH`,
      3. forbidden canonical-label remaps with `AXIS_MISMATCH`,
      4. repeated-label many-to-one canonicalization and later-occurrence
         validation before diagonal normalization.
15. Item extraction failures require `ITEM_REQUIRES_RANK0` tokens when that path is exercised.
16. Tensor indexing validation paths require:
   1. `INDEX_OUT_OF_BOUNDS` tokens for normalized `At`/`Take` coordinates that remain outside the selected axis,
   2. `SLICE_STEP_ZERO` tokens and `value` data for zero-step slices in both statically decidable and runtime value-channel cases,
   3. `INDEX_SPEC_REST_DUPLICATE` tokens and `count` data for duplicate `Rest`
      items in both statically decidable and runtime value-channel cases,
   4. `INDEX_SPEC_ARITY_MISMATCH` tokens plus `source_rank` and
      `consuming_items` data for over-consuming specs in both statically
      decidable and runtime value-channel cases,
   5. `BROADCAST_SHAPE_MISMATCH` coverage for incompatible `Take` common-shape
      broadcasting and incompatible scatter-value directed expansion,
   6. successful canonicalization paths for omitted trailing axes, interior and
      zero-width `Rest`, uniform first-`Take` insertion, strict `Take` name
      provenance, empty selections, and broadcast duplicate aggregation,
   7. successful readable-carrier paths for a `TensorView` source,
      same-element `TensorView` values, and `TensorView[i64]` `Take`
      coordinates, while `set_at!` keeps its destination concrete and mutable.
Tensor determinism coverage additionally requires:
   1. repeated harness observations of one schedule-sensitive floating result
      under a fixed execution identity compared by exact result bits;
   2. every independently activated default-profile result compared to its
      deterministic baseline under dtype tolerance rather than exact baseline
      bits;
   3. strict-profile supported paths compared to their applicable strict
      baseline bits;
   4. future interpreted runtime evidence to bind the artifact and plan
      identities used for each observation;
   5. the required fixture's exact `+0.0` strict-success marker or exact `2^-30`
      strict-error marker checked against `strict_unavailable_path`, with the
      latter accepted only when the conditional unavailable fixture repeats the
      same normalized operation and satisfies its code/data contract.
17. Conditional-required paths:
   1. strict-profile unavailability:
      1. if precondition `strict_unavailable_path` is active, suites require `DETERMINISM_UNAVAILABLE` tokens and profile/backend/device data keys,
      2. v0.1 baseline assertion channel for this path is runtime value channel (`expect.value.kind: Err` with `expect.value_diagnostics`),
      3. baseline suites MUST NOT assert this path via `expect.compile_error`,
      4. source-level strict requests use typed `profile=DeterminismProfile::Strict`, not string profile names,
      5. if precondition is not active, this path is excluded from baseline failure.
   2. extern-linkage runtime probe:
      1. if precondition `ffi_linkage_probe` is active, suites require `EXTERN_LINKAGE_FAILED` tokens and `symbol`/`abi` data keys,
      2. if precondition is not active, this path is excluded from baseline failure.
   3. console write-failure probe:
      1. if precondition `console_write_failure_probe` is active, the harness
         provides deterministic first-write failure sinks for stdout and
         stderr while preserving an out-of-band result channel,
      2. if precondition is not active, this path is excluded from baseline
         failure.
   4. scoped close-failure probe:
      1. if precondition `scoped_close_failure_probe` is active, the harness
         routes reserved byte-writer paths through the deterministic
         close-failure adapter defined by the
         [conditional probe contract](../CONFORMANCE.md),
      2. if precondition is not active, this path is excluded from baseline
         failure.
18. Multi-file module suites require `MODULE_PATH_COMPONENT_INVALID`,
   `MODULE_PATH_MISMATCH`, `MODULE_ROOT_RESERVED`, and `VISIBILITY_VIOLATION`
   tokens when those paths are exercised.
   1. Required v0.1 file-set fixtures MUST follow the normative [file-set invocation contract](../language/modules-resolution.md#file-set-invocation-contract).
   2. A fixture with two or more `.eglur` sources MUST declare its logical root and MUST NOT rely on root inference from layout, entry path, or source headers.
   3. Schema validation MUST cover missing, non-canonical and out-of-range roots before source compilation.
   4. A manifest-less fixture with sources under multiple path prefixes MUST verify that the declared root, rather than a longest-common-ancestor inference, determines canonical module paths.
   5. Package and file-set fixtures with a derived component that is not directly
      source-addressable MUST fail with `MODULE_PATH_COMPONENT_INVALID`, require
      exact `name` and zero-based `index` data, and assert the zero-width
      `bytes[0..0)` primary span on the canonically first offending source unit
      and its leftmost invalid component.
   6. Invalid-component coverage MUST include punctuation, Unicode, the lone `_`,
      a reserved source keyword, and `self` in first-component import-root
      position. Successful coverage MUST include contextual spellings where the
      grammar treats them as identifiers, including `self` after the first
      component.
   7. Package and file-set fixtures with a user source rooted at `std` MUST fail
      with `MODULE_ROOT_RESERVED`, require exact diagnostic data `name = std`,
      and assert primary blame on the canonically first offending source unit.
      At least one fixture MUST also verify precedence over an invalid later path
      component and an invalid `module` verification header.
   8. Successful imports from `std::...` MUST continue to exercise the
      implementation-owned standard-library namespace.
   9. Ordinary unqualified expression-name lookup with no candidate in any
      resolution tier requires `NAME_NOT_FOUND`; context-specific and failed
      import-path lookups retain their more specific diagnostics.
   10. Visibility coverage requires:
      1. cross-module construction, read, and write through public stored fields,
      2. public property accessors and receiver/concrete-owner inherent calls over private storage,
      3. `VISIBILITY_VIOLATION` for private field reads/writes, private property reads/direct accessor calls, and private inherent method calls,
      4. a public member not to escape a private owner,
      5. `VISIBILITY_VIOLATION` before struct-field completeness checks when any required stored field is private,
      6. an inaccessible inherent family to block visible-trait fallback,
      7. declaration-site public type closure for free functions, declaration and method generic bounds, stored fields, properties, inherent and trait methods, enum payloads, constants, aliases, conformance headers and bounds, and associated bindings,
      8. private transparent aliases with public expansions, including concretely discharged private alias bounds, to remain valid across public contracts without hiding symbolic caller obligations, and
      9. public nominal types to retain private representation without recursively exposing that representation as a public contract,
      10. package-mode foreign nominal and trait contracts to require an exact canonical-target `pub import` in the contract-owning module,
      11. aliased and chained public re-exports to preserve foreign canonical identity and remain usable through a direct dependency's public path even when the target owner is transitive-only,
      12. private imports, sibling-module re-exports, public transparent aliases or imports targeting them, and downstream direct dependencies not to substitute for that contract-local bridge, and
      13. imported or re-exported transparent aliases to preserve their expanded struct's inherent method identity and visibility without exposing private method families.
   11. Effect-summary coverage across module boundaries requires:
      1. a known localizable-state summary to preserve the non-bang
         local-mutation exception,
      2. transitive `IO` or `NONDET` to reject that exception with
         `EFFECT_VIOLATION`,
      3. import aliases and public re-exports to preserve the canonical target
         summary.
19. Package-mode module suites require direct-dependency visibility boundary:
   1. imports from direct dependencies are accepted when manifest/package-mode preconditions are met,
   2. imports from transitive-only dependencies fail with `IMPORT_NOT_FOUND`,
   3. bare and quoted TOML dependency-key spellings that decode to the same valid source import root resolve identically,
   4. import-root collisions between direct dependency keys and same-root local module prefixes fail with `IMPORT_ROOT_CONFLICT`, and
   5. a public re-export remains importable through a direct dependency's module path when its canonical target owner is transitive-only, without making the transitive dependency key an import root.
20. Package-root validation suites require:
   1. forced package-mode build without top-level `Eglur.toml` fails with `PACKAGE_MANIFEST_MISSING`,
   2. package-mode build without `src/` root fails with `PACKAGE_SRC_ROOT_MISSING`,
   3. malformed manifest metadata fails with
      `PACKAGE_MANIFEST_PACKAGE_TABLE_MISSING`, `PACKAGE_MANIFEST_NAME_MISSING`,
      `PACKAGE_MANIFEST_NAME_INVALID`, `PACKAGE_MANIFEST_VERSION_MISSING`, or
      `PACKAGE_MANIFEST_VERSION_INVALID`; version coverage rejects leading-zero
      components, and invalid-name coverage distinguishes
      non-string, empty, and invalid-character reasons,
   4. registry manifest coverage requires:
      1. canonical qualified package locators and manifest-local registry aliases
         both normalize to one `RegistryPackageId`,
      2. malformed alias tables fail with `PACKAGE_REGISTRY_ALIAS_INVALID`,
      3. malformed or incomplete registry/package locators fail with
         `PACKAGE_REGISTRY_SELECTOR_INVALID`,
      4. missing, non-string, or malformed exact versions fail with
         `PACKAGE_DEP_VERSION_INVALID`, including leading-zero components, while
         recognized version-range syntax
         fails with `PACKAGE_VERSION_RANGE_UNSUPPORTED`;
   5. dependency entries with an invalid closed source form fail with
      `PACKAGE_DEP_SOURCE_CONFLICT`, including mixed path/registry sources and
      absent source intent;
   6. manifest path ingress coverage rejects empty, absolute/drive/UNC, and
      backslash spellings with `PACKAGE_DEP_PATH_INVALID` and exact declared-path
      data,
   7. lexical normalization coverage proves equivalent repeated-separator,
      dot-segment, and parent-segment spellings produce one exact
      `CanonicalPathSelector`,
   8. filesystem coverage distinguishes absent paths, non-directory targets,
      broken symbolic links, and symbolic-link cycles with
      `PACKAGE_DEP_PATH_UNRESOLVED`,
   9. a resolved directory missing `Eglur.toml` or `src/` retains
      `PACKAGE_MANIFEST_MISSING` or `PACKAGE_SRC_ROOT_MISSING` with selecting
      dependency/path context,
   10. two direct keys reaching one path package by lexical or symbolic-link
       alias fail with `PACKAGE_DEP_DUPLICATE`, while transitive convergence from
       different declaring packages remains valid,
   11. package dependency cycles fail with `PACKAGE_DEP_CYCLE` after aliases are
       interned by resolved package root,
   12. distinct package instances with one `PackageName`, including the root,
       fail with `PACKAGE_NAME_CONFLICT`; coverage distinguishes `source` and
       `version_and_source` reason data where constructible,
   13. package-mode build without top-level `Eglur.lock` fails with
       `PACKAGE_LOCK_MISSING`,
   14. malformed or noncanonical complete graph locks fail with
       `PACKAGE_LOCK_INVALID`, including invalid top-level/node/edge shape,
       noncanonical `LockedNodeId` discovery order, dangling or unreachable
       nodes, source-incompatible edges, noncanonical lock paths, malformed
       versions or `tree-sha256:` values, and contradictory registry
       coordinates,
   15. a structurally valid graph lock that disagrees with reached manifests or
       resolved path topology fails with `PACKAGE_LOCK_OUT_OF_SYNC`; coverage
       distinguishes `package_metadata`, `dependency_key_set`, and
       `dependency_selector`, including a stale edge declared by a reached
       transitive package and path false splits and false merges,
   16. a manifest may use a valid noncanonical path spelling, but a lock path is
       accepted only in exact `CanonicalPathSelector` form; noncanonical lock
       text is `PACKAGE_LOCK_INVALID`, not lock staleness,
   17. representative combined-failure probes assert normative phase precedence
       (manifest metadata, registry aliases, dependency keys, dependency entries,
       and lexical selectors before lock validation; lock structure before
       manifest synchronization; canonical tree validation before integrity),
   18. representative graph-order probes assert first-discovery canonical
       dependency-key DFS, graph-wide lock completeness, and convergence
       independent of manifest declaration order,
   19. representative lockfile-order probes assert mismatch-category precedence
       (`package_metadata` before `dependency_key_set` before
       `dependency_selector`) and canonical dependency-key tie-breaks,
   20. invalid decoded dependency keys in manifests and lockfiles fail with
       `PACKAGE_DEP_KEY_INVALID`,
   21. key-domain coverage includes punctuation-bearing bare keys, quoted Unicode
       keys, source keywords, relative-import roots, and the reserved `std` root,
   22. dependency-key validation precedes selector errors, missing-lock errors,
       and lock synchronization mismatches, with canonical decoded-key selection
       when multiple invalid keys exist,
   23. registry acquisition and tree coverage requires:
       1. ordered provider scripts distinguish missing releases from transport
          failure after partial emission; a tree-invalid emitted prefix does not
          replace the primary transport diagnostic,
       2. only compiler-owned sink accounting derives portable logical-limit
          failure; fixtures do not inject a precomputed limit outcome,
       3. exact maxima and the first value beyond each of the seven portable
          logical limits are covered, including adjacent mixed-limit cases that
          assert the normative per-emission check order and one limit failure
          before a later transport-failure finish,
       4. missing releases, provider transport failure, and portable logical-limit
          failure remain distinct as `PACKAGE_REGISTRY_RELEASE_MISSING`,
          `PACKAGE_REGISTRY_TRANSPORT_FAILED`, and
          `PACKAGE_REGISTRY_LIMIT_EXCEEDED`, without exposing partial entries,
       5. a complete logical entry source whose canonical regular-file records
          match the locked `tree-sha256:` digest is accepted,
       6. malformed raw `sha256:` lock spelling fails with `PACKAGE_LOCK_INVALID`,
       7. raw non-UTF-8 paths, unsupported entry kinds, duplicate regular files,
          and other tree-domain failures fail with
          `PACKAGE_REGISTRY_TREE_INVALID` before hashing,
       8. Unicode 17.0 simple-case-fold path collisions fail while paths equal
          only under full folding or Unicode normalization remain distinct,
       9. a valid canonical tree whose digest differs from the lock fails with
          `PACKAGE_REGISTRY_INTEGRITY_MISMATCH`,
       10. mixed-fault fixtures distinguish every adjacent primary-reason step
          from `invalid_path_encoding` through `file_ancestor`,
       11. the separate `registry-cache-v1` profile references these
           single-invocation fixtures to verify cache lifecycle behavior without
           inventing a source diagnostic or treating source-fixture success as
           atomicity, recovery, or concurrency evidence,
   24. registry graph identity coverage requires:
       1. one `RegistryPackageId` cannot select two exact versions and fails with
          `PACKAGE_VERSION_CONFLICT`,
       2. qualified and aliased selectors for one release converge and duplicate
          direct bindings fail with `PACKAGE_DEP_DUPLICATE`,
       3. path and registry sources never implicitly converge and retain
          `PACKAGE_NAME_CONFLICT` when their declared names collide,
   25. addressed compiler input includes exact declarative symbolic-link topology
       and ordered transactional registry provider scripts, including raw byte
       sources, duplicate multiplicity, entry kinds, targets, contents, compact
       repetitions, and provider finish signals, so filesystem-alias and
       registry-validation outcomes are reproducible.
21. Numeric and domain runtime error paths require:
   1. `INT_OVERFLOW` / `INT_DIV_BY_ZERO` / `INT_REM_BY_ZERO` tokens plus required
      `operation` and `dtype` data keys; promotion, integer reduction, and
      scatter-add overflow probes assert exact fixed operation/dtype identities;
   2. `CAST_INVALID_VALUE` tokens plus required `src_dtype` and `dst_dtype` data keys;
   3. `CAST_INVALID_DTYPE_PAIR` tokens plus required `src_dtype` and `dst_dtype` data keys;
   4. for non-const-evaluable cast failures, primary outcome MUST be `value.kind: Err` (not `runtime_error`);
   5. for const initializer cast paths, `CONST_INIT_NOT_CONST` MUST take primary precedence over cast-failure codes when initializer is non-const-evaluable;
   6. `DOMAIN_INVALID_VALUE` tokens plus required `domain` and `value` data keys;
   7. domain scalar coverage includes both statically decidable contextual literal failures and non-const-evaluable conversion failures returning `Err(DomainError)`;
   8. fixed-width integer literal coverage includes:
      1. unconstrained call literals defaulting to `i64`,
      2. expected-result and explicit type-argument contexts selecting a narrow dtype,
      3. out-of-range contextual or defaulted literals failing with `INT_OVERFLOW`,
      4. unresolved overload candidates not supplying literal expected types or bounded-width ranking;
   9. scalar checked-arithmetic coverage includes:
      1. successful dynamic arithmetic returning `Ok` from `checked {}`,
      2. dynamic overflow, division-by-zero, and remainder-by-zero returning `Err(NumericError)` with unchanged cause codes,
      3. `NUMERIC_CHECKED_CONTEXT_REQUIRED` for a potentially faulting dynamic scalar operator outside `checked`,
      4. const-evaluable failures retaining their `INT_*` cause code rather than the checked-context code,
      5. explicit scalar wrapping arithmetic remaining available outside `checked`,
      6. a `checked` boundary not extending into nested closure bodies,
      7. a `checked` body not inheriting `?` authorization from a surrounding `result` block;
   10. scalar and tensor integer division/remainder coverage includes:
      1. quotient truncation toward zero and dividend-signed remainder for every signed operand-sign combination,
      2. signed `MIN / -1` failing with `INT_OVERFLOW`,
      3. signed `MIN % -1` and tensor `rem(MIN, -1)` succeeding with zero;
   11. integer tensor coverage includes:
      1. checked overflow for unary `neg`/`abs`, `sum`, default `reduce`,
         `contract`, and `einop` execution paths through `Err(NumericError)`,
      2. reduction fixtures that distinguish the canonical left fold from
         reassociated tree reduction and final-only range checking,
      3. reduced-axis normalization independent of caller axis-selector order,
      4. mixed-width contract/einop contributions accumulated in the promoted
         result dtype,
      5. exact `sum`, `reduce`, `contract`, and `einop` operation identities for
         checked reduction overflow;
   12. integer scatter-add coverage includes:
      1. overflow at an intermediate duplicate update even when later updates would cancel it,
      2. `set_at!` rollback of updates completed before a later overflow,
      3. exact `operation=scatter_add` and destination `dtype` identity;
   13. unary tensor transcendental coverage includes:
      1. `exp`, `log`, `sqrt`, and `tanh` accepting floating and complex inputs
         while preserving every required inexact dtype,
      2. every member rejecting representative `Bool`, signed-integer, and
         unsigned-integer inputs through value-channel `Err(DTypeError)`,
      3. real negative-domain results, NaN propagation, zero/infinity limits,
         and finite `tanh` limits at infinity following the normative IEEE
         value path without `NumericError`,
      4. representative complex `log` and `sqrt` principal values on both
         signed-zero sides of the negative-real branch cut.
22. Numeric promotion suite paths require:
   1. required dtype outcomes for promotion cases;
   2. `INT_OVERFLOW` for an implicit conversion failure before arithmetic;
   3. `INT_OVERFLOW` for promoted-dtype arithmetic after both conversions
      succeed;
   4. exact promoted `dtype` and selected primitive `operation` identities for
      both failure stages.
23. Runtime-abort channel tests require `RUNTIME_ABORT` tokens and `source` data key.
   1. When baseline semantics normatively fix the abort `data.source` value, required suites SHOULD assert that exact value via `data_equals`.
24. Binding/constant declaration suites require:
   1. `IMMUTABLE_BINDING_ASSIGN` for immutable `let` reassignment targets (local bindings and immutable `field` members);
   2. assignment target and value validation requires:
      1. `ASSIGN_TARGET_NOT_ASSIGNABLE` for representative non-lvalue assignment targets such as unresolved bare identifier assignment, call results, and `?` expressions,
      2. `ASSIGNMENT_TYPE_MISMATCH` for annotated local and constant
         initializers, mutable-local reassignment, mutable-field assignment,
         and explicit-dereference assignment; assert canonical `expected_type`
         and `actual_type` and the incompatible value's primary span,
      3. preserve target/property/mutability precedence, expression-internal
         diagnostics, annotation-before-destructuring order, and suppression
         of derivative errors from poisoned initializers,
      4. compatible values, aliases and direct `Never` compatibility remain
         accepted; this code does not introduce implicit conversions;
   3. `VAR_DECL_FORBIDDEN` for any declaration use of `var`;
      1. this requirement is phase-invariant (`parse` vs `type` detection does not change code);
   4. `STRUCT_FIELD_DEFAULT_FORBIDDEN` for struct field declaration default initializer forms;
   5. `CONST_INIT_NOT_CONST`, `CONST_FORWARD_REFERENCE`, and `CONST_CYCLE` where corresponding constant-validation paths are exercised;
   6. `STRUCT_INIT_UNKNOWN_FIELD`, `STRUCT_INIT_DUPLICATE_FIELD`, `STRUCT_INIT_MISSING_FIELD`, and `STRUCT_INIT_PROPERTY_FORBIDDEN` where record-initializer validation paths are exercised (struct literals and enum named-payload constructors);
   7. `EFFECT_SIGNATURE` where method receiver bangness/mutability signature rules are exercised;
   8. `METHOD_RECEIVER_INVALID` where method declarations omit a receiver, place `self` outside first position, or use a receiver type outside `Self` / `&Self` / `&mut Self`;
   9. `SELF_TYPE_INVALID` where contextual `Self` appears outside owner-bound methods and associated-type binding RHS contexts;
   10. `NON_BANG_MUT_PARAM_FORBIDDEN` where non-bang mutable-parameter signatures are exercised;
   11. `PROP_ACCESSOR_MISSING`, `PROP_ACCESSOR_SIGNATURE_MISMATCH`, and `PROPERTY_ASSIGN_FORBIDDEN` where property declaration/update paths are exercised;
   12. `NAME_AMBIGUOUS` where duplicate struct member names are exercised across the shared `field`/`prop` member namespace, and where a bare match pattern name collides with a scrutinee enum variant that must be written with enum qualification;
   13. explicit-reference validation requires:
      1. `REF_ESCAPE_FORBIDDEN` where a reference escapes v0.1 local storage or
         enters a local binding whose lexical extent can outlive a possible
         referent,
      2. `REFERENCE_TARGET_INVALID` where `&` does not name a readable stable
         local place or `&mut` does not name a writable stable local place;
   14. closure capture paths require:
      1. immutable value capture at closure construction, escape, copy, and repeated invocation preserve value semantics,
      2. `MUTABLE_CAPTURE_FORBIDDEN` for read-only capture of a `let mut` binding and writes rooted in any captured binding,
      3. an explicit immutable rebinding is the accepted freeze boundary for a mutable local,
      4. nested closure-parameter capture follows the same immutable snapshot rule,
      5. a non-escaping closure may capture an immutable explicit-reference binding, while mutable-binding capture and writes through a captured `&mut T` remain primary over reference escape and effect diagnostics,
      6. inline task-thunk capture snapshots immutable values during `spawn!` argument evaluation and rejects mutable or reference captures before scheduling,
      7. a preconstructed pure `Fn() -> T` task thunk retains its construction-time environment without recapture by `spawn!`;
   15. `RETURN_OUTSIDE_FUNCTION`, `RETURN_VALUE_REQUIRED`, `RETURN_VALUE_FORBIDDEN`, and `RETURN_TYPE_MISMATCH` where return-statement, expression-body, and block-result typing/scope paths are exercised, including closure body return rejection;
   16. let/parameter pattern validation paths require:
      1. `PARAM_PATTERN_UNSUPPORTED` for non-identifier function, method, trait, and extern parameter patterns,
      2. `LET_PATTERN_UNSUPPORTED` for local binding patterns outside identifier, wildcard, Unit pattern `()`, and recursive tuple patterns,
      3. `LET_PATTERN_TYPE_MISMATCH` for Unit local binding patterns applied to non-`Unit` values, or tuple local binding patterns applied to non-tuple values or wrong tuple arity,
      4. `LET_PATTERN_DUPLICATE_BINDING` for duplicate identifiers introduced by one local binding pattern.
   17. call argument binding, constructor form, and typing validation paths require:
      1. `CALL_POSITIONAL_AFTER_NAMED`,
      2. `CALL_DUPLICATE_NAMED_ARG`,
      3. `CALL_UNKNOWN_NAMED_ARG`, including a named argument on a `Fn`-typed value whose underlying declaration uses the same parameter name,
      4. `CALL_MISSING_ARG`,
      5. `CALL_TOO_MANY_ARGS`,
      6. `CALL_ARG_TYPE_MISMATCH`,
      7. `CONSTRUCTOR_TARGET_KIND_MISMATCH` for uniquely resolved struct and
         variant-constructor heads whose semantic target kind is invalid for
         the written surface,
      8. `CONSTRUCTOR_PAYLOAD_FORM_MISMATCH` for unit, zero-arity
         positional, positional, and named constructor declarations, with form
         validation preceding payload arity, named fields, payload expression
         typing, and generic owner inference;
   18. tuple projection validation paths require:
      1. `TUPLE_PROJECTION_NON_TUPLE` for numeric projection on a non-tuple receiver,
      2. `TUPLE_PROJECTION_OUT_OF_BOUNDS` for numeric projection outside the static tuple arity.
   19. type-argument validation paths require:
      1. `TYPE_ARG_INFERENCE_FAILED` for underconstrained or partial `::[...]`
         call usage and for underconstrained, ambiguous, or inconsistent generic
         nominal-constructor inference,
      2. `TYPE_ARG_BRACKET_SYNTAX_FORBIDDEN` for ordinary `f[T](...)` generic-call attempts,
      3. `TYPE_ARG_ARITY_MISMATCH` for representative generic type-alias arity
         mismatch, `Never[T]`, and zero-arity associated-type application,
      4. dedicated tensor constructor dtype brackets preserve eligibility
         through import aliases and re-exports, lose it under local shadowing,
         normalize transparent aliases, and reject unsupported normalized types
         with `CALL_ARG_TYPE_MISMATCH`,
      5. generic struct and enum constructors infer from direct field/payload
         constraints, accept exact expected owner types from annotations,
         returns, and statically selected parameters for phantom, zero-field,
         and unit-variant construction, and use `CALL_ARG_TYPE_MISMATCH` when a
         complete vector fixes a slot whose initializer is incompatible;
         constructor-specific
         `TYPE_ARG_INFERENCE_FAILED` precedes an enclosing unannotated binding's
         general `BINDING_TYPE_UNDETERMINED` path,
      6. constructor-head bracket and turbofish type arguments are rejected with
         `PARSE_UNEXPECTED_TOKEN` while an annotated expected owner remains the
         explicit construction path; ordinary owner, variant, and
         field/payload-shape diagnostics precede constructor inference,
      7. transparent aliases may supply expected owner types but do not introduce
         constructor heads; a uniquely resolved alias head uses
         `CONSTRUCTOR_TARGET_KIND_MISMATCH`, while an unresolved head retains
         `NAME_NOT_FOUND`,
      8. constructor inference validates bounds only after obtaining a complete
         vector and does not select a phantom parameter from available bound
         witnesses,
      9. callable inference may bind a parameter to a complete normalized error
         union, never decomposes a structural union for member-wise evidence,
         accepts an independently or explicitly fixed nested parameter after
         ordinary union assignability, and produces source-order-invariant
         `TYPE_ARG_INFERENCE_FAILED` diagnostics when such a parameter remains
         underconstrained.
   20. type-alias coverage requires:
      1. transparent alias equality/assignability behavior,
      2. generic alias substitution behavior,
      3. accepted alias expansion whose target contains a `Res` error-channel union,
      4. cycle rejection with `TYPE_ALIAS_CYCLE`.
   21. abstract numeric kind value-position validation requires `ABSTRACT_NUMERIC_KIND_VALUE_POSITION` for representative parameter, return, local/field annotation, and generic concrete argument positions.
   22. numeric operand validation paths require `NUMERIC_OPERAND_NOT_NUMERIC`
       for representative unary and binary non-numeric operands.
   23. logical operator validation paths require `LOGICAL_OPERAND_TYPE_MISMATCH` for representative non-`Bool`/`Tri` operands.
   24. comparison and hashing validation paths require:
      1. successful canonical `Eq` dispatch for manual and derived implementations,
      2. successful numeric-promotion, tuple, `Opt`, `Res`, `Seq`, and `Vec` equality,
      3. successful canonical `Ord.cmp` dispatch for manual and derived implementations, relational operators, standard sorting, numeric promotion, generic `Ord` bounds, and floating total order,
      4. successful satisfaction of the fixed `Ord -> Eq` prerequisite for both concrete impl declarations and generic bounds,
      5. `COMPARISON_TYPE_MISMATCH` for representative equality generic-argument mismatch, a same-static-type value without `Eq`, a missing generic `Eq` bound, a nested conditional-carrier obligation failure, explicitly non-equality-comparable `Task`, `Own`, reference, and callable values, relational operand-type mismatch, a same-static-type value without `Ord`, a missing generic `Ord` bound, and relational operands promoted to complex,
      6. `TRAIT_BOUND_NOT_SATISFIED` when an `Ord` implementation target does not implement `Eq`,
      7. successful source calls to manual and derived `Hash.hash(&value, seed)` implementations,
      8. successful independent `Hash` implementations on types without `Eq`,
      9. successful conditional `Hash` implementations for scalar, tuple, `Opt`, `Res`, `Seq`, and `Vec` values,
      10. deterministic repeated hash results for the same value and seed without asserting an exact numeric output,
      11. `TRAIT_IMPL_MISSING_METHOD` and `TRAIT_IMPL_SIGNATURE_MISMATCH` for incomplete or incorrectly shaped `Hash` implementations.
   25. interpolated-string validation paths require:
      1. `FSTRING_UNRESOLVED_NAME`,
      2. `FSTRING_INVALID_PLACEHOLDER`.
   26. placeholder/hole validation paths require `PLACEHOLDER_INVALID_CONTEXT` when a `HoleExpr` appears outside placeholder closure shorthand or hole-based partial application.
   27. `ArrayExpr` and `MapExpr` container-binding paths require:
      1. `BINDING_TYPE_UNDETERMINED` for unresolved empty literal typing (`let x = []` with no expected type),
      2. `METHOD_NOT_FOUND` when Vec-only APIs are invoked on `ArrayExpr` without explicit `to_vec()` conversion,
      3. syntax-directed distinction among `[]`, `[:]`, non-empty arrays, and non-empty maps without expected-type parsing,
      4. `BINDING_TYPE_UNDETERMINED` for an unresolved empty map literal,
      5. `TRAIT_BOUND_NOT_SATISFIED` when the inferred or expected map key type lacks `Eq` or `Hash`,
      6. left-to-right key/value evaluation, insertion-order preservation, and replacement by later equal literal keys without a hidden failure channel.
   28. union-type validation requires:
      1. representative success coverage for `Res[T, Union[E1, E2]]` error-channel propagation,
      2. `UNION_TYPE_ARITY_MISMATCH` for a reserved `Union` head with fewer than two type arguments,
      3. `UNION_TYPE_UNSUPPORTED` for representative non-`Res` error-channel positions,
      4. `RETURN_TYPE_MISMATCH` when `?` would propagate an error type outside the result block's declared error union.
   29. `Never` type and continuation validation requires:
      1. canonical identity through direct spelling, prelude import, and
         transparent alias expansion without a value constructor,
      2. directional compatibility with independently established expected
         types in returns, arguments, bindings, callable results, and branch
         joins,
      3. no constructor lifting for collections or residual carriers and no
         inferred trait implementations,
      4. exact local generic inference of `T = Never` only from a direct
         first-order parameter position when no earlier exact constraint fixes
         `T`,
      5. exact unresolved-overload applicability, including a `Never` argument
         selecting only a `Never` parameter,
      6. preservation of no-normal-continuation through strict contexts and
         expected-type compatibility, with all written source still validated,
      7. `Union` normalization that removes exact `Never` members and produces
         `Never` when no member remains,
      8. calls deriving continuation only from the selected normalized callable
         result, without body-level divergence inference,
      9. no automatic trait, layout, ABI, or structural-uninhabited-to-bottom
         inference.
   30. raw pointer type and relay validation requires:
      1. `RAW_POINTER_TYPE_UNSUPPORTED` for representative source-written non-extern type positions, including an explicit local annotation inside `unsafe`,
      2. success coverage for direct extern-to-extern relay, inferred immutable locals, repeated local reads, all no-binding discard forms, nested-`unsafe` preservation, same-region conditional propagation, and `*mut T` to extern `*const T` weakening,
      3. `RAW_POINTER_USE_UNSUPPORTED` for representative region/callable results, mutable and aggregate storage, inferred generic instantiation, ordinary non-extern calls, capture, comparison, and dereference paths,
      4. required `use` and `type` data plus exact `use` values on `RAW_POINTER_USE_UNSUPPORTED` fixtures,
      5. `CALL_ARG_TYPE_MISMATCH` when an otherwise valid extern sink would require `*const T` to `*mut T` conversion or a different pointee type,
      6. `IF_BRANCH_TYPE_MISMATCH` and `MATCH_ARM_TYPE_MISMATCH` when pointer-producing branches do not have one exact raw-pointer type, without applying extern-argument weakening to the branch join.
   31. closure bindings with unannotated parameters remain local to their
       initializer and fail with `BINDING_TYPE_UNDETERMINED`; no later call
       provides backward inference through a local binding.
   32. lexical local-scope coverage requires:
      1. nearest-active-binding lookup and restoration across nested blocks,
         callable and closure bodies, loop bodies, and match-arm blocks,
      2. initializer lookup against only previously active bindings, including
         outer-binding shadow initializers, missing self-initializers, and no
         local declaration hoisting,
      3. binder non-escape after loops, match arms, and closures,
      4. `LOCAL_BINDING_REDECLARATION` for same-block declarations and duplicate
         callable or closure parameters, including precedence over initializer
         failures,
      5. reference storage/provenance and ownership obligations remaining keyed
         to binding identity across spelling shadowing.
25. `std::vec` bounds suites require:
   1. `slice` end-exclusive success, empty-slice success, and invalid range/end `IndexError` value-channel paths,
   2. `insert` success at `i == len`, `insert` failure at `i > len`, and `remove` failure at `i >= len`,
   3. bang `set!` / `insert!` / `remove!` / `truncate!` failure paths preserve the receiver without partial mutation.
26. Overload suites require:
   1. `OVERLOAD_MARKER_REQUIRED` when same-name declarations omit explicit `overload`;
   2. `OVERLOAD_VISIBILITY_MISMATCH` when a module-level or inherent overload family mixes public and private declarations;
   3. `OVERLOAD_RETURN_ONLY_DIFFERENCE` for return-type-only overload differences;
   4. `OVERLOAD_OVERLAP` for non-disjoint overload declarations;
   5. `OVERLOAD_AMBIGUOUS` when a call leaves multiple applicable overload
      candidates or expected-`Fn` coercion leaves multiple compatible overload
      candidates;
   6. `CALLABLE_VALUE_NO_MATCH` when expected-`Fn` coercion of an overload set
      leaves no compatible candidate. This is distinct from a call whose typed
      argument vector matches no overload.
27. Ownership-flow suites in v0.1 conformance baseline assume ownership surface enabled and require:
   1. per-path discharge mismatches require `OWNERSHIP_PATH_UNBALANCED`, including bang bodies, direct/conditional local roots, lexical normal exits, nested `yield`, inner `result`/`checked` early exits, and loop-control edges; enclosing-region roots remain live when an inner edge does not exit their obligation region;
   2. forbidden nested or ordinary-generic ownership positions require `OWNERSHIP_TYPE_POSITION_FORBIDDEN`;
   3. named-root transfer without `move` requires `OWNERSHIP_TRANSFER_REQUIRES_MOVE`;
   4. discard, overwrite, or `_ = move x` requires `OWNERSHIP_DISCARD_FORBIDDEN`;
   5. expression-level `move` misuse on non-root operands requires `MOVE_REQUIRES_OWN`;
   6. expression-level `own` misuse on ownership-root operands requires `OWN_REQUIRES_NON_OWN`;
   7. `own` on a non-local/non-parameter identifier requires `OWN_REQUIRES_LOCAL_BINDING`;
   8. `drop_own` without explicit `move` requires `DROP_OWN_REQUIRES_MOVE`;
   9. `drop_own move` on a designated resource root requires `RESOURCE_DROP_FORBIDDEN`;
   10. repeated consume of one root on the same typed path, including a second consuming resource terminal, requires `OWNERSHIP_DOUBLE_MOVE`;
   11. use/borrow of a non-definitely-owned root, any local reference copy/merge derived from such a root, or a source consumed by `own` requires `OWNERSHIP_USE_AFTER_DISCHARGE`;
   12. ownership analysis enforces path-sensitive merge (`Owned`/`Consumed`/`MaybeConsumed`) and rejects `MaybeConsumed` at ownership-use sites;
   13. ownership diagnostics use the explicit one-primary-per-failing-function override in `../language/ownership-boundary.md`; same-class candidates select the earliest canonical source location rather than analysis traversal order, and implementations SHOULD attach additional distinct cause locations via `related` notes/spans;
   14. guarded `match` arms require both guard-true and guard-false ownership path coverage without transferring a tentative payload to a rejected arm; a typed exit leaving a guard with a possible present payload requires `OWNERSHIP_PATH_UNBALANCED`;
   15. conditional carriers require `(move r)?` or `match move r` for named roots and introduce a direct root only on `Some`/`Ok` paths;
   16. generic containers and callable values, including closure and partial-application capture environments, remain ownership-free in v0.1;
   17. consuming a tentative ownership binding in a match guard requires `OWNERSHIP_GUARD_CONSUME_FORBIDDEN`, while a shared guard borrow remains valid;
   18. `Own[T]` does not generally forward methods from `T` or implement `Clone`; cloning a root requires `METHOD_NOT_FOUND`, while designated resource methods resolve only on their explicit `Own[R]` wrapper types.
   19. associated type bindings remain ownership-free and reject direct or conditional-carrier ownership bindings with `OWNERSHIP_TYPE_POSITION_FORBIDDEN`; an alias that hides an ownership root is already rejected at its alias declaration and is not claimed as a distinct associated-binding path.
   20. ownership-position validation remains primary when an `Own[...]` payload, generic argument, or associated binding also violates the reference-free abstraction-slot rule.
   21. two owning bindings on one alternative of a conditional-root match
       pattern require `PATTERN_OWNERSHIP_BINDING_OVERLAP`; one owning binding
       may coexist with ownership-free bindings.
28. Implementations MAY expose non-baseline ownership-disabled mode; if such mode is tested, ownership syntax/type-surface paths (including `Own[...]`) require `OWNERSHIP_PROFILE_DISABLED`.
29. `CAPABILITY_*` diagnostics are extension-profile codes and are outside required v0.1 conformance unless capability features are explicitly enabled.
30. ADT baseline suites require:
   1. `ENUM_VARIANT_VISIBILITY_FORBIDDEN` for per-variant visibility attempts;
   2. `TYPE_RECURSIVE_LAYOUT_REQUIRES_INDIRECTION` for direct, mutual, mixed, generic-wrapper, transformed-generic, associated-projection-mediated, tuple, `Opt`, and `Res` inline-layout cycles not broken by `Box` or another recognized boundary;
      1. cycle selection asserts the earliest cyclic storage occurrence; optional related text SHOULD show a deterministic cycle path,
      2. distinct generic substitutions must not be merged into false cycles,
      3. representative runtime-sized boundaries include `Box`, `Seq`, `Task`, and `Fn`;
   3. `RESERVED_BUILTIN_NAME` for built-in or contextual type-namespace redefinition/shadowing attempts (`Opt`, `Res`, `Own`, `Box`, `Fn`, `Union`, `Never`, `Self`) and value-constructor redefinition attempts (`Some`, `None`, `Ok`, `Err`);
      1. representative type-namespace coverage includes a nominal declaration, type alias, and generic type parameter,
      2. reservation errors precede recursive-layout analysis of the same invalid declaration;
   4. inhabitance-aware match coverage requires:
      1. `MATCH_NON_EXHAUSTIVE` when guarded arms do not provide unconditional
         case coverage or a zero-arm match is not proven uninhabited,
      2. successful zero-arm matches over empty enums and recursive no-base
         sums,
      3. impossible payload variants excluded from required coverage and
         rejected with `MATCH_UNREACHABLE_ARM` when explicitly matched,
      4. conservative rejection for `Unknown` generic/projection cases,
      5. representative base-case sums and always-inhabited empty-container
         fields that prevent false uninhabited classification;
   5. named enum payload constructor validation reuses:
      1. `STRUCT_INIT_UNKNOWN_FIELD`,
      2. `STRUCT_INIT_DUPLICATE_FIELD`,
      3. `STRUCT_INIT_MISSING_FIELD`;
   6. source-enum and catalog-sum constructor and pattern usage is
      owner-qualified (`Owner::Variant`):
      1. unit, positional, and named-payload user-enum forms are accepted,
      2. representative catalog-sum forms are accepted,
      3. dot forms such as `Flag.On` and `Tri.Unknown` fail with
         `NAME_NOT_FOUND`,
      4. direct free-name imports of user-enum and catalog-sum variants fail
         with `IMPORT_NOT_FOUND`,
      5. after longest-module-prefix resolution, a qualified braced expression
         accepts a direct public struct type path without changing the
         owner-qualified named-variant surface;
   7. derive provider model paths require:
      1. built-in or implementation-trusted provider acceptance through same `derive(...)` path resolution rules,
      2. v0.1 baseline provider set is closed (no source/manifest registration activation path),
      3. active provider set is fixed before type checking, and derive expansion traversal order is deterministic (module path, declaration source order, derive-entry order),
      4. `DERIVE_ELIGIBILITY_FAILED` for provider-specific payload or bound rejection,
      5. `DERIVE_EXPANSION_FORBIDDEN` is extension-profile-only in v0.1 and excluded from required baseline suites,
      6. positive direct and mutual recursive derive candidates are accepted by the finite derive-local greatest fixed point when every dependency leaving the candidate component succeeds,
      7. candidate failure propagates through recursive dependencies while an ordinary non-candidate obligation cycle remains `TRAIT_BOUND_NOT_SATISFIED`,
      8. cyclic generic substitutions may reorder, repeat, omit, or replace source parameters with closed types, but nesting a source parameter below a type-forming node requires `DERIVE_ELIGIBILITY_FAILED` with `reason = recursive_type_argument_growth`;
   8. struct inheritance-clause rejection requires `INHERITANCE_FORBIDDEN` for parse-valid `struct Name : Type { ... }` declarations.
   9. derive/attribute-lite paths require:
      1. `ATTRIBUTE_UNSUPPORTED` for attribute names outside v0.1 supported families (`derive`, enum-variant `default`, function-level `test`/`ignore`);
      2. `ATTRIBUTE_TARGET_UNSUPPORTED` for supported attribute names on unsupported targets when no more specific target diagnostic applies;
      3. `DERIVE_TARGET_UNSUPPORTED` for derive on non-`struct`/`enum` semantic targets;
      4. `DERIVE_DUPLICATE_ENTRY` for repeated derive entries in one list;
      5. `DERIVE_TRAIT_RESOLUTION_FAILED` for unresolved/non-trait/non-visible derive paths;
      6. `DERIVE_TRAIT_UNSUPPORTED` for resolved traits with no active derive support;
      7. `DERIVE_TRAIT_UNSUPPORTED` for prelude traits that are visible but intentionally outside the v0.1 built-in derive allowlist (`Display`, `From`, `TryFrom`, `Error`);
      8. `DERIVE_ELIGIBILITY_FAILED` for provider-specific payload/bound rejection, including generic payloads missing a required explicit source bound;
      9. `DERIVE_EXPANSION_FORBIDDEN` is extension-profile-only in v0.1 and excluded from required baseline suites;
      10. enum `derive(Default)` eligibility paths use `DERIVE_ELIGIBILITY_FAILED` with default-marker failure tokens (`missing`, `multiple`, `unit`) according to failing rule;
      11. test-attribute paths require:
         1. `TEST_ATTRIBUTE_TARGET_UNSUPPORTED` for `#[test]` on non-top-level-function targets,
         2. `TEST_FUNCTION_INVALID_SIGNATURE` for `#[test]` function signature
            violations, including an explicit `Never` result,
         3. `TEST_IGNORE_REQUIRES_TEST` for `#[ignore]` declarations lacking same-item `#[test]`.
      12. extension slots tracked by `LANG-OI-019` in `../open-items/language.md`
          and described in `../../roadmap/language/attributes.md` are excluded
          from required v0.1 conformance.
      13. `Hash` derive paths require:
         1. the exact seeded prelude method to be source-callable,
         2. acceptance without a same-type `Eq` implementation,
         3. `DERIVE_ELIGIBILITY_FAILED` when a participating field lacks `Hash`.
      14. recursive derive coverage requires:
         1. positive direct and mutual candidate components,
         2. failure propagation from an unsupported participating field,
         3. accepted generic parameter permutation and closed replacement,
         4. rejected recursive type-argument growth,
         5. preservation of the ordinary solver's no-cycle-as-proof rule.
      15. equal derive candidates produced by distinct attribute occurrences remain distinct for ordinary coherence and require `TRAIT_IMPL_DUPLICATE` rather than being silently deduplicated.
31. Tensor shape and transform schema suites require:
   1. `BROADCAST_SHAPE_MISMATCH` in both compile and value channels for statically decidable and runtime-dependent failures of baseline common-shape broadcasting and directed expansion.
   2. `WITH_SIZES_UNKNOWN_KEY` for:
      1. const-evaluable unknown scalar keys through `compile_error`,
      2. named axis-pack identifiers used as binding keys through `compile_error`,
      3. non-const-evaluable unknown keys through schema-invocation `Err(...)` diagnostics,
   3. `AXIS_PACK_ANON_DUPLICATE` when one transform literal repeats anonymous `...` pack capture,
   4. `AXIS_PACK_UNBOUND` when output-side named packs are not bound by
      input-side named packs and when an `einop` output uses an anonymous pack,
   5. closed structural mapping coverage requires:
      1. right distribution and inverse factoring to assert nonuniform values
         under the tagged coordinate bijection,
      2. recursive distribution, repeated-factor lineage, and empty-branch
         behavior to follow the written proof tree,
      3. a product of two same-spelled sum factors to select the left-factor
         outer proof when left and right distribution both match,
      4. an explicitly written sum-branch permutation to compose with the
         selected distributive proof and determine output order,
      5. affine left distribution to remain a valid `view`, while a non-affine
         admitted distribution fails with `NOT_A_VIEW` rather than copying,
      6. an affine `view` and a materializing `rearrange` to preserve a carried
         source name when distribution retains one source physical-axis origin
         in exactly one output physical axis,
      7. a distributive-looking map with mismatched factor lineage to fail with
         `AXIS_MAPPING_UNSUPPORTED` even when resolved extents and global numel
         agree,
      8. a supported distributive map to proceed to dimension validation and
         report `INCONSISTENT_DIMS` when its resolved source extent is wrong,
      9. explicit non-overlapping multi-output partitioning across a mixed
         sum/product tree to remain accepted, and
      10. structurally admitted factoring across distinct input roots to reach
          segment validation and report `SEGMENT_STRADDLE`, while explicit
          cross-input concatenation followed by unary factoring succeeds,
   6. closed dimension-solver coverage for:
      1. exact univariate nonlinear roots and no-root
         `INCONSISTENT_DIMS`,
      2. zero-size multiple-solution families and isolated multivariate-product
         `AMBIGUOUS_DIMS`,
      3. `DIM_BINDING_REQUIRED` for unresolved multivariate components,
      4. partial `.with_sizes(...)` bindings that reduce a multivariate
         component to the univariate baseline,
      5. large concrete dimensions without value-proportional search.
   7. `AMBIGUOUS_DIMS` / `INCONSISTENT_DIMS` for pack-partition ambiguity/inconsistency under the same channel rules as other dim-solver failures.
   8. readable-carrier paths that pass `TensorView` directly to
      representative unary, binary, conditional, wrapping, cast, default
      reduce, contract, and `einop` operations and verify owning `Tensor`
      results without an explicit input materialization step.
   9. transform callable result paths that verify:
      1. one-output `view` returns `TensorView`,
      2. multi-output `view` returns a right-hand-side-ordered tuple of
         `TensorView` values,
      3. `inflate` accepts `TensorView` directly and returns owning `Tensor`,
      4. multi-input `rearrange` accepts readable carriers and assigns each
         right-hand-side output the common canonical dtype of its contributor
         inputs,
      5. rank-0 operand/output positions and repeated occurrences of one generic
         carrier projection retain deterministic provenance and dtype,
      6. separate `rearrange` outputs may retain different dtypes, while one
         output with mixed concrete or distinct unresolved contributor
         projections fails with `REARRANGE_OUTPUT_DTYPE_MISMATCH`,
      7. right-hand-side output order selects the first dtype mismatch and
         asserts `output_index`, `left_type`, and `right_type`,
      8. statically decidable numel and segment failures retain precedence over
         the output dtype relation.
   10. bounded tensor-layer boundary coverage requires:
      1. transform contextual spellings to remain ordinary declarations and
         calls outside complete schema-literal grammar,
      2. a complete schema literal to retain intrinsic meaning despite a
         same-spelled local or module declaration,
      3. `AxisNameLit` and `AxisIndexLit` to construct canonical
         `std::tensor` types despite same-spelled user type declarations.
   11. multiplicative `AxisExpr` coordinate coverage requires:
      1. nonuniform value fixtures for both split and merge directions with the
         rightmost factor varying fastest,
      2. logical product order remaining independent of physical source
         strides,
      3. a supported recursive mixed sum/product mapping plus written-tree-directed
         distribution and inverse factoring,
      4. stable repeated-occurrence handling, including branch-local lineage
         for a distributed repeated factor,
      5. output-only `inflate` factors in both factor positions following the
         same product order,
      6. unary and cross-input `einop` product outputs following the same
         logical coordinate order,
      7. left-zero and nested right-zero product domains producing empty maps
         without division,
      8. consumed-axis and output-only mathematical extent overflow rejecting
         rather than wrapping,
      9. cross-input `rearrange` product grouping failing with
         `SEGMENT_STRADDLE` even when global numel matches,
      10. cross-input factoring reaching the same segment boundary only after
          its written structural proof succeeds,
      11. empty distributive branches using total empty coordinate maps, and
      12. affine and non-affine distributive `view` outcomes remaining distinct.
   12. v0.1 `einop` coverage requires:
      1. a nested tagged-sum `+` on the left-hand side and on the right-hand side
         to fail with `AXIS_MAPPING_UNSUPPORTED`,
      2. schema-level rejection before dimension solving or Core Tensor IR
         lowering, and
      3. existing atomic and product-only `einop` forms to remain accepted,
      4. surviving identifiers, explicitly sized output-only identifiers, and
         fresh input/output integer-literal occurrences to have distinct
         value-level coverage,
      5. named packs to align memberwise, anonymous input packs to reduce, and
         anonymous output packs to fail with `AXIS_PACK_UNBOUND`,
      6. repeated input labels to use nonuniform diagonal values and dimension
         mismatch to fail with `INCONSISTENT_DIMS`,
      7. repeated identifiers or named-pack tokens in one output to fail with
         `EINOP_OUTPUT_LABEL_DUPLICATE`,
      8. N-ary dtype selection to occur before direct operand conversion and
         multiplication, without a narrower prefix operation,
      9. unary `bool` result types to distinguish a structural output from a
         schema-static reduction-capable output, including a mixed-dtype
         multi-output tuple,
      10. a nonuniform multi-input/multi-output fixture to assert every output's
          independent values and right-hand-side order,
      11. a later-output checked-arithmetic failure to fail the whole
          expression without exposing an earlier successful output, and
      12. behavioral fixtures to distinguish the required reshape, diagonal,
          alignment, cast-before-multiply, one ordered reduction, grouping, and
          owning-realization semantics wherever those stages affect observable
          values, dtypes, shapes, failures, or ownership. Suites MUST NOT assert
          a particular primitive trace beyond the logically observable order
          allowed by `CONFORMANCE.md`.
32. Tensor reduction suites require:
   1. canonical source-form commitment to reject an argument list that does
      not match `ReduceByArgs` with `REDUCE_BY_ARGUMENTS_INVALID`, without
      reserving an ordinary user method named `reduce_by`;
   2. ordered-phase `reduce_by` failure paths:
      1. `REDUCE_BY_PHASE_NON_REDUCED_AXIS` when any phase references a kept/non-reduced axis,
      2. `REDUCE_BY_PHASE_OVERLAP` when one reduced axis is assigned to multiple phases,
      3. `REDUCE_BY_PHASE_INCOMPLETE` when phase axis-union does not fully cover reduced axes.
   3. ordered-phase failure fixtures assert the required data projection from
      `diagnostic-contracts.yaml`; release tooling rejects a duplicate or
      divergent fixture-level key contract.
   4. primitive `mean` dtype behavior:
      1. floating and complex input success preserves input dtype,
      2. `bool` and integer inputs fail through value-channel `Err(DTypeError)`.
   5. empty-domain behavior:
      1. primitive and schema sum paths use additive identities in their resolved
         result dtypes, including positive floating and complex zero,
      2. real and complex `mean` produce dtype-preserving NaN components for
         every existing output coordinate,
      3. an empty primitive axis selection is an owning no-op and `keepdims`
         preserves the ordinary value, dtype, shape, and overlay contract
         without bypassing operation dtype validation,
      4. a zero-length retained axis succeeds with the required empty output
         when selected volume is nonzero or the operation has a defined
         empty-domain result,
      5. const-evaluable empty `max` asserts compile-channel
         `REDUCTION_EMPTY_DOMAIN`,
      6. runtime-dependent empty `max` returns `Err(ShapeError)` with the same
         code,
      7. empty-output shape does not suppress empty-`max` failure, and the
         diagnostic reports exact `operation` and normalized first zero `axis`
         in both channels,
      8. empty `contract` and `einop` sum stages use the additive identity in the
         promoted result dtype, preserve empty retained outputs, and apply the
         rule independently to each `einop` output,
      9. custom `reduce_by` receives empty phase subtensors, uses callback
         `Ok` values without an injected identity, and propagates callback
         `Err` values without wrapping; an empty retained output invokes the
         callback zero times.
   6. custom reducer callable typing and results:
      1. single reducer form may change the input element dtype and returns an owning tensor,
      2. ordered form accepts one or more complete phases,
      3. ordered phases may form a heterogeneous dtype chain and each phase receives the previous phase's successful payload type,
      4. a phase whose callback input does not match the previous phase output fails with `CALL_ARG_TYPE_MISMATCH`,
      5. callback successful payloads outside the documented `DType[T]` family fail with `CALL_ARG_TYPE_MISMATCH`,
      6. exact callback error types are unioned with `TransformError`, and phase errors propagate without wrapping,
      7. runtime schema failures use `TransformError`,
      8. effectful phase callbacks fail with `PURE_COMBINATOR_EFFECTFUL_CALLBACK`,
      9. a complete direct invocation may provide `T_0` to an input-dependent
         inline callback before its body is checked,
      10. canonical `then` and unary-tuple `apply` desugarings provide the same
          expression-local input context,
      11. a stored schema with an underdetermined inline callback fails at its
          initializer with `BINDING_TYPE_UNDETERMINED`,
      12. a stored schema with construction-site-complete callback types is an
          ordinary reusable monomorphic value,
      13. invocation-provided input context does not invent an unresolved
          callback result/error type,
      14. a generic named callback remains subject to construction-site
          `TYPE_ARG_INFERENCE_FAILED`,
      15. an ordinary user-defined method named `reduce_by` remains an ordinary
          member call, including when its argument is a phase-shaped tuple,
      16. schema-only whitespace axis syntax on a non-`ReduceSchema` receiver
          fails with `REDUCE_BY_ARGUMENTS_INVALID` after receiver resolution,
      17. canonical `ReduceSchema.reduce_by` does not fall back to ordinary
          call grammar when its arguments do not match `ReduceByArgs`, and
          fails with `REDUCE_BY_ARGUMENTS_INVALID`,
      18. a compatible later invocation does not repair an underdetermined
          stored schema initializer,
      19. an immediate heterogeneous phase chain propagates each successful
          payload type into the next inline callback input,
      20. axis-partition validation precedes callback type and effect validation
          for the same schema invocation, including an effectful callback on an
          overlapping phase.
33. Prelude and `std::str` baseline formatting suites require:
   1. `Display.to_str` method and generic prelude `to_str` adapter parity for representative baseline scalar types,
   2. a user-defined `Display` implementation observed consistently through method call, free adapter, and `format` placeholder,
   3. required scalar formatting coverage for:
      1. finite `f32` / `f64` canonical float display, including the `.0` cue for integral-valued finite floats,
      2. positive and negative floating zero as `0.0` and `-0.0`,
      3. floating special values as `nan`, `inf`, and `-inf`.
   4. tensor `DType[T]` formatting parity between `dtype.to_str()` and the generic prelude adapter,
   5. representative pure string-surface coverage for:
      1. `len_bytes`,
      2. `is_empty`,
      3. `contains`,
      4. `starts_with`,
      5. `ends_with`,
      6. `trim` / `trim_start` / `trim_end`,
      7. `replace`,
      8. `lines`,
      9. `split` (empty-token-preserving),
      10. `split_nonempty` (empty-token-dropping),
      11. `strip_prefix` / `strip_suffix`,
      12. `to_lower_ascii` / `to_upper_ascii`,
      13. `concat`,
      14. `format`,
         1. repeated named placeholders,
         2. doubled literal braces `{{` / `}}`,
         3. unmatched braces through value channel `Err(FormatError)`.
   6. v0.1 baseline asserts string-helper failure paths through value channel (`Res::Err` for `format`) rather than runtime-error channel.
   7. operational error formatting requires exact `Display` coverage for
      `PathError`, `IoError`, `TextError`, `NetError`, `TimeError`, and their
      associated kind types, with method/free-adapter parity inherited from the
      prelude `Display` contract.
   8. `ScopedFailure[E]` formatting, when `E: Display`, requires:
      1. `Primary(error)` to render exactly as `error`, without a phase label,
      2. `PrimaryAndClose { primary, close }` to render the primary error first
         and the close error second using the exact prelude format.
34. `std::io` / `std::time` baseline suites require:
   1. REQUIRED surface-path coverage for successful and failing value-channel outcomes (`Res` with `IoError`, `TextError`, `PathError`, `TimeError`) on representative APIs,
      1. pure path lexical baseline probes MUST include representative coverage for:
         1. `Path.is_absolute`,
         2. `Path.join`,
         3. `Path.parent`,
         4. `Path.file_name`,
         5. `Path.extension`,
         6. path normalization/canonicalization and native-bridge extension APIs are deferred non-baseline in v0.1 and MUST NOT be required by baseline suites,
      2. process-environment baseline probes MUST include representative coverage for:
         1. `process_snapshot!`,
         2. `ProcessSnapshot.invocation_arg`,
         3. `ProcessSnapshot.args`,
         4. `ProcessSnapshot.env_get`,
         5. `ProcessSnapshot.cwd`,
         6. `invocation_arg!`,
         7. `args!`,
         8. `env_get!`,
         9. `cwd!`,
         10. portable source fixtures cover capture/accessor typing, accessor
             stability, and result shapes; exact launcher-provided argument
             membership/order and malformed native process data require the
             isolated implementation-certification environment,
         11. process-environment mutation APIs (`set_env!`, `remove_env!`, `set_cwd!`) are deferred non-baseline in v0.1 and MUST NOT be required by baseline suites,
      3. byte-streaming baseline probes MUST include representative coverage for:
         1. `open_reader!`,
         2. `open_writer!`,
         3. `open_appender!`,
         4. `with_reader!`, `with_writer!`, and `with_appender!` scoped cleanup,
         5. `Own[ByteReader].read_chunk!`,
         6. `Own[ByteWriter].write_all!`,
         7. `Own[ByteWriter].flush!`,
         8. consuming `Own[ByteReader].close!` and `Own[ByteWriter].close!`,
         9. post-close and repeated-close source use fails through ownership diagnostics rather than `IoError`,
         10. random-access byte APIs (for example `seek!`, `read_exact!`, `read_at!`, `write_at!`) are deferred non-baseline in v0.1 and MUST NOT be required by baseline suites,
         11. invalid `Own[ByteReader].read_chunk!` `max_bytes` domain paths use `DOMAIN_INVALID_VALUE`, not `IoError` or runtime-abort channel,
         12. byte-reader failure progress remains a normative implementation
             obligation:
            1. injected evidence for a read that acquires bytes before a
               lower-level failure MUST verify that those bytes are delivered
               in source order before `Err`, that a later successful retry
               neither skips nor duplicates bytes, and that the root remains
               available for its required close attempt,
            2. required v0.1 source fixtures MUST NOT require host read-failure
               injection; successful reads and EOF fixtures MUST NOT be cited as
               evidence for error progress or retry behavior,
         13. incremental byte-writer failure behavior remains a normative
             implementation obligation:
            1. when the implementation can return `Err` after accepting payload
               bytes, injected short-write or late-failure evidence MUST verify
               preservation of the accepted prefix, absence of implicit replay,
               and continued availability of the root for its required close
               attempt,
            2. required v0.1 source fixtures MUST NOT require host fault
               injection; passing only empty-payload and successful-write
               fixtures MUST NOT be cited as evidence for failure progress,
               rollback, or retry behavior,
         14. byte-stream flush and close failure behavior remains a normative
             implementation-certification obligation:
            1. a controlled `flush!` failure after transfer of a non-empty proper
               prefix MUST verify that prefix at the controlled underlying sink,
               retention of the exact pending suffix, continued root ownership,
               suffix-first ordering for later writes, and absence of replay when
               a later flush or consuming close succeeds,
            2. injected reader- and writer-close failures MUST verify that the
               consumed root and corresponding local handle are terminal on both
               `Ok` and `Err`; writer evidence MUST also cover failure before,
               during, and after pending output transfer,
            3. target-adapter instrumentation MUST be capable of detecting a
               retained usable handle, implementation-managed cleanup scheduled
               after return, or an unsafe host-close retry against a reused
               resource identity,
            4. successful flush/close fixtures and compile-time ownership-use
               failures establish only the ordinary source contract and MUST NOT
               be cited as evidence for partial flush continuation or terminal
               host-resource retirement after `Err`,
      4. text-streaming baseline probes MUST include representative coverage for:
         1. `open_text_reader!`,
         2. `open_text_writer!`,
         3. `open_text_appender!`,
         4. `with_text_reader!`, `with_text_writer!`, and `with_text_appender!` scoped cleanup,
         5. `Own[TextReader].read_line!`,
         6. `Own[TextWriter].write_str!`,
         7. `Own[TextWriter].write_line!`,
         8. `Own[TextWriter].flush!`,
         9. consuming `Own[TextReader].close!` and `Own[TextWriter].close!`,
         10. post-close and repeated-close source use fails through ownership diagnostics rather than `TextError`,
         11. invalid UTF-8 decode path surfaces through value channel `Res::Err(TextError)` and MUST NOT use runtime-error channel,
             1. representative suites MUST assert `error_variant: TextError::InvalidUtf8`,
             2. representative filesystem/open failures MUST assert `error_variant: TextError::Io`,
             3. a portable fixture with valid input after a malformed line MUST
                assert that repeated `read_line!` calls remain
                `Err(InvalidUtf8)` rather than skipping to that later input,
         12. text-reader I/O failure progress remains a normative implementation
             obligation:
            1. injected failure after a partial line MUST verify preservation of
               the partial bytes and same-line resumption without omission or
               duplication when a later read succeeds,
            2. injected failure after a complete valid line is available MUST
               verify that the line is returned before `Err(Io(_))`,
            3. required v0.1 source fixtures MUST NOT require host read-failure
               injection; successful line reads and portable decode failures
               MUST NOT be cited as evidence for I/O-error progress,
         13. incremental text-writer failure behavior remains a normative
             implementation obligation:
            1. fault-injected evidence for a reachable post-progress failure
               MUST treat progress as encoded bytes, MUST NOT assume a Unicode
               scalar boundary, and MUST NOT infer newline absence from
               `write_line!` returning `Err`,
            2. required v0.1 source fixtures MUST NOT require a controllable
               short-write sink; successful text writes and decode-failure
               fixtures MUST NOT be cited as evidence for writer progress or
               retry behavior,
         14. text-stream flush and close certification MUST apply the shared
             byte-stream lifecycle evidence to UTF-8 encoded bytes:
            1. partial flush and writer-close transfer boundaries MAY split one
               encoded Unicode scalar and MUST still preserve the exact byte
               suffix without replay,
            2. injected failures surface as `TextError::Io` while preserving the
               shared root-liveness and terminal-retirement guarantees,
            3. an implementation MAY reuse byte-adapter certification evidence
               only when it documents that the text adapter delegates the same
               lifecycle without an independent buffering or release path,
      5. standard-stream baseline probes MUST include representative coverage for:
         1. `stdin!`,
         2. `stdout!`,
         3. `stderr!`,
         4. `with_stdin!`, `with_stdout!`, and `with_stderr!`,
         5. consuming wrapper-local `close!` on one acquired root MUST NOT make a later acquisition of the same standard stream kind fail when the underlying process stream remains available,
         6. `print!` and `eprint!` successful calls return `Ok(())`,
         7. under `console_write_failure_probe`, `print!` and `eprint!` write failures return `Err(Io(_))` with `error_variant: TextError::Io` rather than the runtime-abort channel,
         8. the required first-write rejection probe establishes the zero-prefix
            value-channel path only; partial-write and post-write release
            failures remain implementation-certification obligations, and
            passing the probe MUST NOT be cited as evidence for general prefix
            or replay behavior,
         9. standard-stream lifecycle certification requires injected evidence
            that acquisition `Err` terminally retires only a provisional wrapper,
            flush failure preserves ordered pending bytes, and close `Ok` or
            `Err` terminally retires the consumed wrapper without closing or
            invalidating the underlying process stream; later successful wrapper
            acquisition is the required survival oracle,
         10. controlled simultaneous text-write and wrapper-release failures
             from `print!` / `eprint!` MUST return the exact write error through
             raw `TextError`, MUST still establish wrapper retirement, and MUST
             NOT expose the later release error as `ScopedFailure`; the
             first-write rejection probe does not establish this precedence,
      6. io path/file baseline probes MUST include representative coverage for:
         1. `exists!` / `entry_exists!`,
            1. both return `Ok(true)` for a directly addressed regular file and `Ok(false)` for a missing final entry,
            2. `exists!` follows the final component and returns `Ok(false)` for a dangling link, while `entry_exists!` does not follow it and returns `Ok(true)`,
         2. `stat!` / `entry_stat!`,
            1. both report regular-file positive byte size and non-file canonical zero byte size for directly addressed entries,
            2. `stat!` follows the final component and returns `Err(IoError)` for a dangling link, while `entry_stat!` reports `FileKind::Symlink`,
            3. required v0.1 source fixtures cover directly constructible regular, directory, and missing paths; the dangling-link distinction remains normative but MUST NOT be required from source fixtures because symbolic-link creation is not baseline,
         3. `create_dir!`:
            1. directly constructible fixtures cover successful single-component creation, existing final-directory and regular-file rejection with the existing entry preserved, and missing-parent rejection without parent creation,
            2. deterministic rejection fixtures establish only failures before final host creation and MUST NOT be cited as evidence that every outward `Err(IoError)` leaves the target absent,
            3. implementation-certification injection after final host creation MUST accept only an absent target or the complete requested directory, MUST reject auxiliary or unrelated namespace changes, and MUST establish that the error exposes no outcome or phase discriminator,
            4. retry evidence MUST establish one host-create attempt per source-level call and fresh current-namespace evaluation rather than automatic replay or continuation; a quiescent retry after completed creation returns existing-entry `Err(IoError)` and cannot certify the first call's outcome,
            5. required v0.1 source fixtures MUST NOT depend on lost-acknowledgement, concurrent namespace mutation, or injected host failures; passing deterministic success and rejection fixtures MUST NOT be cited as evidence for the post-invocation failure envelope,
         4. `create_dir_all!`:
            1. directly constructible fixtures cover recursive creation, repeated idempotent success, regular-file blocker rejection with the blocker preserved, and a fresh retry after removal of that blocker,
            2. deterministic blocker fixtures establish only rejection at an observed non-directory component and MUST NOT be cited as evidence for partial recursive creation or late failure,
            3. implementation-certification injection at every component boundary MUST accept zero through all operation-created directories only when, disregarding independent concurrent mutation, they form a prefix of the call's missing-component creation sequence; it MUST reject removal or replacement of pre-existing entries, changes to pre-existing non-directory payloads, rollback, auxiliary entries, and unrelated namespace changes, while permitting existing directory state to reflect allowed child insertion,
            4. controlled full-creation-before-error evidence MUST permit `Err(IoError)` with the complete requested directory tree and MUST establish that the error exposes no count, path, phase, or completion discriminator,
            5. retry evidence MUST establish one ordered component-creation sequence per source-level call and fresh current-namespace resolution rather than automatic replay; in a quiescent namespace, a retry accepts the surviving directory prefix and continues from the first missing component,
            6. required v0.1 source fixtures MUST NOT depend on permission mutation, concurrent namespace mutation, lost acknowledgement, or injected host failures; passing deterministic success, blocker, and retry fixtures MUST NOT be cited as evidence for partial-progress or late-failure behavior,
         5. `remove_entry!`:
            1. a directly addressed regular file is removed, a missing final entry returns `Err(IoError)`, and a directly addressed directory is rejected with the directory preserved,
            2. final-entry no-follow removal of file links, directory links, and dangling links, plus direct `FileKind::Other` removal, remains a normative implementation obligation,
            3. deterministic success, missing-path, and wrong-kind fixtures establish only complete removal or definite rejection and MUST NOT be cited as evidence that every outward `Err(IoError)` leaves the selected entry unchanged,
            4. implementation-certification injection at the final-entry host removal boundary MUST permit `Err(IoError)` only with an unchanged namespace before commit or the same complete removal as `Ok(())` after commit; it MUST reject entry recreation, another-name removal, target or payload modification, auxiliary entries, and compensating rollback, and MUST establish that the error exposes no outcome or phase discriminator,
            5. retry evidence MUST establish fresh current-namespace resolution and one source-level removal attempt rather than automatic replay or continuation; in a quiescent namespace, retry after completed removal returns missing-entry `Err(IoError)` without certifying the first call's outcome,
            6. required v0.1 source fixtures MUST NOT depend on symbolic-link or `FileKind::Other` construction, concurrent replacement, lost acknowledgement, or injected host failures; passing portable deterministic fixtures MUST NOT be cited as evidence for no-follow races, post-invocation failure state, or retry identity,
         6. `remove_dir!`:
            1. a directly addressed empty directory is removed; a missing path returns `Err(IoError)`, while a rejected regular file or non-empty directory and its contents are preserved,
            2. final symbolic links and `FileKind::Other` entries are rejected without following or modifying a link target,
            3. deterministic success, missing-path, and wrong-kind fixtures establish only complete removal or definite rejection and MUST NOT be cited as evidence that every outward `Err(IoError)` leaves the selected entry unchanged,
            4. implementation-certification injection at the final-entry host removal boundary MUST permit `Err(IoError)` only with an unchanged namespace before commit or the same complete empty-directory removal as `Ok(())` after commit; it MUST reject entry recreation, descendant removal, another-name removal, auxiliary entries, and compensating rollback, and MUST establish that the error exposes no outcome or phase discriminator,
            5. retry evidence MUST establish fresh current-namespace resolution and one source-level removal attempt rather than automatic replay or continuation; in a quiescent namespace, retry after completed removal returns missing-entry `Err(IoError)` without certifying the first call's outcome, while replacement-race evidence MUST preserve kind enforcement and final-entry no-follow behavior,
            6. required v0.1 source fixtures MUST NOT depend on symbolic-link or `FileKind::Other` construction, concurrent replacement, lost acknowledgement, or injected host failures; passing portable deterministic fixtures MUST NOT be cited as evidence for no-follow races, post-invocation failure state, or retry identity,
         7. `remove_dir_all!`:
            1. a directly addressed nested file/directory tree is removed, repeated removal of a missing final entry returns `Ok(())`, and a final regular file is rejected,
            2. a final symbolic link is rejected without modifying the link or its target, while nested symbolic links are removed as entries without traversing their targets,
            3. recursive no-follow behavior under concurrent replacement remains a normative race-safety obligation and MUST NOT be implemented as metadata preflight followed by an ordinary target-following recursive routine,
            4. required v0.1 source fixtures MUST NOT depend on constructing symbolic links or `FileKind::Other` entries because neither creation surface is baseline; passing only directly constructible file/directory/missing fixtures MUST NOT be cited as evidence for the symbolic-link, `FileKind::Other`, or replacement-race obligations,
            5. `Err(IoError)` may follow partial descendant removal without rollback, while every attempted removal retains the recursive no-follow guarantee,
            6. retry is defined against the current namespace and MUST NOT be characterized as replay against the failed call's original tree snapshot,
            7. required v0.1 source fixtures MUST NOT require permission mutation, concurrent namespace replacement, mounted-filesystem manipulation, or injected host failures; passing deterministic success and missing-path retry fixtures MUST NOT be cited as evidence for partial-failure progress or recovery behavior,
         8. shared `DestinationPolicy` behavior for `rename!` and `copy!`:
            1. omission selects `DestinationPolicy::RejectExisting`, while replacement is requested only through the named argument `destination=DestinationPolicy::ReplaceCompatible`,
            2. directly constructible regular-file fixtures exercise default rejection and explicit compatible replacement for both operations,
            3. a final destination symbolic link is always an entry rather than a followed target; symbolic-link, `FileKind::Other`, and concurrent-creation obligations remain normative implementation requirements,
            4. required v0.1 source fixtures MUST NOT depend on constructing symbolic links or `FileKind::Other` entries, and a passing existence-preflight fixture MUST NOT be cited as evidence for race-safe no-replace behavior,
         9. `rename!`:
            1. directly constructible fixtures cover missing-to-present movement, default existing-destination rejection, explicit regular-file replacement, same-identity no-op, compatible empty-directory replacement, kind mismatch, and non-empty destination rejection,
            2. source/destination final-link identity, dangling-link movement, `FileKind::Other`, and cross-filesystem rejection without copy-and-delete remain normative implementation obligations,
            3. controlled default existing-destination rejection MUST preserve
               both source and destination and establishes only the definite
               pre-commit rejection path,
            4. passing deterministic rejection and success fixtures MUST NOT be
               cited as evidence that every outward `Err` leaves the namespace
               unchanged or that caller retry is idempotent,
            5. injected evidence for a host operation that commits the complete
               rename before reporting failure MUST verify that `rename!`
               returns `Err(IoError)` without rollback, copy-and-delete, or an
               operation-induced post-state other than the complete rename;
               this remains an implementation-certification obligation,
            6. required v0.1 source fixtures MUST NOT require symbolic-link,
               cross-filesystem, remote-filesystem, or lost-acknowledgement
               setup because none of those host-fixture capabilities is
               baseline,
         10. `copy!`:
            1. every call supplies the required named argument `source_links=LinkMode::Follow` or `source_links=LinkMode::Reject`; omission fails with `CALL_MISSING_ARG`,
            2. directly constructible regular-file fixtures exercise successful source acquisition under both modes and assert copied byte count and destination content,
            3. directly constructible fixtures cover default existing-destination rejection, explicit replacement, same-entry rejection without truncation, final destination-directory rejection, and missing/directory source rejection under both source modes,
            4. source `LinkMode::Follow` target resolution, source `LinkMode::Reject` final-link rejection and race safety, hard-link identity, and source `FileKind::Other` rejection remain normative implementation obligations,
            5. required v0.1 source fixtures MUST NOT depend on constructing symbolic or hard links or `FileKind::Other` entries because none of those creation surfaces is baseline; passing only direct regular/missing/directory fixtures MUST NOT be cited as evidence for link identity, source-link, `FileKind::Other`, or replacement-race obligations,
            6. deterministic stable-source fixtures establish only the successful byte count and destination content for an unchanged acquired source; passing them MUST NOT be cited as evidence for a point-in-time snapshot, an initial-size bound, or concurrent-source behavior,
            7. implementation-certification evidence for source-path replacement after acquisition MUST establish that an in-progress call remains attached to the one acquired underlying object rather than reopening or retargeting through `from`,
            8. controlled concurrent-overwrite, truncation, and growth evidence MAY accept any result consistent with an increasing-offset stream through the observed end of file on that same acquired object; it MUST NOT require one deterministic mutation interleaving or treat a differing pre-call or post-call source size as failure when `Ok(n)` matches the copied content,
            9. required v0.1 source fixtures MUST NOT depend on concurrent mutation, scheduler control, or an injected source-acquisition seam; passing portable deterministic fixtures MUST NOT be cited as evidence for the non-snapshot source-consistency obligations,
            10. deterministic existing-destination, incompatible-kind, source-rejection, and self-copy fixtures establish only failures before destination modification and MUST assert that the directly observable destination remains unchanged,
            11. implementation-certification fault injection after destination creation, truncation, or compatible-entry removal MUST accept only the policy-appropriate absent, unchanged, or regular-prefix states and MUST reject a stale old suffix, bytes from another source object, a modified final-link target, or a surviving auxiliary staging or backup entry,
            12. a controlled failure after the complete payload is written but before close or success acknowledgement MUST permit `Err(IoError)` with complete destination content and MUST establish that no copied-byte count or phase discriminator is exposed through the error,
            13. conformance MUST NOT require `ReplaceCompatible` to preserve or replace an existing regular-file object's identity; when a hard-link fixture makes the strategy observable, either direct modification of the old object or installation of a new object is conforming if the final destination content and all other copy rules hold,
            14. retry evidence MUST establish one output attempt per source-level call, offset-zero source reacquisition, current-destination policy evaluation, `RejectExisting` rejection of a surviving prefix entry, and `ReplaceCompatible` restart rather than prefix resumption,
            15. required v0.1 source fixtures MUST NOT depend on injected write, close, acknowledgement, retry, or resource-retirement faults or on hard-link construction; passing deterministic success or pre-modification rejection fixtures MUST NOT be cited as evidence for partial-progress, late-failure, object-identity, retry, terminal-retirement, or simultaneous-failure obligations,
            16. implementation-certification lifecycle evidence MUST model source and destination resources independently and establish the nested managed-resource contract:
                1. a source acquired before destination rejection or acquisition failure is terminally retired before return, while any provisional destination resource is retired by its acquisition operation,
                2. after both acquisitions succeed, destination close occurs before source close on either typed transfer result, each acquired resource receives exactly one logical terminal release, and source close is still attempted after destination-close failure,
                3. the outward error is the first observed failure in source-acquisition, destination-acceptance/acquisition, transfer, destination-close, source-close order; controlled combinations MUST include destination-acceptance/acquisition failure plus source-close failure, transfer failure plus both close failures, successful transfer plus both close failures, and successful transfer and destination close plus source-close failure,
                4. the combinations in Rule 16.3 MUST respectively expose the destination-acceptance/acquisition, transfer, destination-close, and source-close errors, while successful transfer and both successful closes return `Ok(n)`; no `Err(IoError)` exposes the otherwise complete byte count,
            17. terminal-retirement evidence MUST establish through a target-appropriate handle census, lock or exclusive-access probe, or equivalent out-of-band observation that neither `Ok` nor `Err` leaves a usable source or destination handle, reference, or hidden later cleanup; source-level rename, remove, or reopen success alone MUST NOT be cited as universal proof,
            18. streamed and optimized host-copy paths MUST satisfy the same lifecycle and first-error contract; certification MUST NOT require identical syscalls or distinct physical handles when an optimized path supplies equivalent identity, result, destination-state, and terminal-retirement evidence,
         11. `read_bytes!` and shared whole-file-read behavior:
            1. deterministic source fixtures MUST cover quiescent empty and
               non-empty byte results and missing-target failure; helper-family
               fixtures MUST collectively cover successful direct regular-file
               reads under both `LinkMode` values,
            2. implementation-certification evidence MUST use a controlled
               adapter or schedule to establish one successful acquisition, a
               byte-zero initial cursor, monotonically increasing observation,
               and no path re-resolution or source-object switch within the call,
            3. source-path replacement or removal after acquisition MUST leave
               the in-progress call attached to the acquired object; evidence
               MUST reject bytes obtained by reopening or retargeting through
               `path`,
            4. controlled same-object overwrite, truncation, and growth evidence
               MAY accept any byte result consistent with one increasing-offset
               observation through the first observed end of file, including a
               sequence that was never the complete contents at one instant,
               but MUST reject omission, duplication, reordering, or bytes made
               available only after that end-of-file result,
            5. when an implementation obtains file-size metadata, controlled
               evidence MUST establish that the value is at most an allocation
               hint and does not cap the result; growth observed before the first
               end of file MAY extend the result, while conformance MUST NOT
               impose a completion deadline on a continuously growing source,
            6. injected read failure after internal byte progress MUST return the
               exact read error without exposing bytes, a progress count, a
               lifecycle phase, or the acquired-object identity, and MUST still
               establish terminal retirement,
            7. controlled close-only failure MUST replace body success, while
               simultaneous read-body and close failures MUST return the exact
               body error, establish terminal retirement, and omit the later
               close error from the raw domain-error result,
            8. retry evidence MUST establish fresh `LinkMode` resolution,
               acquisition of the then-current selected object, and an offset-zero
               read for each caller invocation rather than automatic replay or
               continuation from an unread suffix,
            9. required v0.1 source fixtures MUST NOT depend on path or content
               races, scheduler control, metadata substitution, fault injection,
               or symbolic-link construction; passing deterministic fixtures
               MUST NOT be cited as evidence for source identity, mutation,
               progress, close, or retry obligations,
         12. `write_bytes!` and shared whole-file-write behavior:
            1. deterministic source fixtures cover absent-target creation, replacement of longer existing contents by a shorter payload without a stale suffix, and empty-payload truncation,
            2. implementation-certification injection after creation, truncation, partial write, full write, flush, and close MUST accept only the unchanged, absent, or empty/partial/complete prefix states permitted for that starting target and `LinkMode`,
            3. injected evidence MUST reject a stale old suffix, compensating removal or restoration, a modified `LinkMode::Reject` link target, or an auxiliary staging or backup entry after return,
            4. a controlled late failure after the complete payload MUST permit `Err` with complete target content and MUST establish that the error exposes no progress count or phase discriminator,
            5. hard-link and final-symbolic-link evidence MUST preserve direct target identity and the selected `LinkMode`: truncation is observable through hard links, `Follow` retains the final link entry while modifying its target, and `Reject` never modifies a final link target,
            6. retry evidence MUST establish one create-or-truncate lifecycle per source-level call and full-payload restart at byte zero rather than prefix resumption or automatic whole-call replay,
            7. required v0.1 source fixtures MUST NOT depend on injected acquisition, write, flush, close, or acknowledgement failures or on hard-link or symbolic-link construction; passing deterministic success fixtures MUST NOT be cited as evidence for partial-progress, late-failure, target-identity, or retry obligations,
            8. controlled simultaneous write-body and close failures MUST
               return the exact write-body error through the raw domain-error
               type, MUST still establish terminal retirement, and MUST NOT
               expose the later close error as `ScopedFailure`; source-level
               success fixtures do not establish this precedence;
         13. `read_text!` / `write_text!`,
            1. whole-file text helper failures surface through value-channel `Res::Err(TextError)` and MUST NOT use runtime-error channel,
            2. whole-file text helper failure suites MUST assert `TextError::Io` vs `TextError::InvalidUtf8` with `expect.value.error_variant`,
            3. implementation-certification evidence for `read_text!` MUST
               establish that byte reading reaches its first end of file before
               UTF-8 decoding begins; an injected read failure after an invalid
               byte prefix returns `TextError::Io` rather than
               `TextError::InvalidUtf8`,
            4. after a complete byte read, invalid UTF-8 plus a simultaneous close
               failure returns `TextError::InvalidUtf8`; body success plus a sole
               close failure returns `TextError::Io`,
            5. UTF-8 encoding from `Str` is total, so `write_text!` MUST expose only `TextError::Io` on typed failure; `TextError::InvalidUtf8` remains a read/decode result,
            6. injected partial-write evidence MAY observe a target byte prefix that is not independently valid UTF-8,
         14. `read_lines!` / `write_lines!`,
            1. `read_lines!` uses the same line splitting semantics as `Str.lines`,
            2. `read_lines!` inherits the complete source, lifecycle, failure,
               and retry behavior of `read_text!` before applying `Str.lines`,
            3. whole-file line helper failures use `TextError` consistently with `read_text!` / `write_text!`,
            4. successful `write_lines!` fixtures MUST assert the exact concatenated text payload, including one trailing `\n` for every input line and truncation of longer old contents,
            5. `write_lines!` typed failures are only `TextError::Io`, and injected partial-write evidence MAY end inside a scalar encoding or at either side of an appended `\n`,
         15. `list_paths!` / `list_files!` / `list_dirs!`:
            1. nested-directory results use joined child paths and empty directories return an empty sequence for all three functions,
            2. representative ordering covers mixed ASCII case, non-ASCII scalars, and canonically equivalent composed/decomposed spellings without requiring the host filesystem to preserve one particular spelling,
            3. controlled file/directory fixtures establish that `list_paths!` returns every direct child, `list_files!` returns only `FileKind::File`, and `list_dirs!` returns only `FileKind::Dir`,
            4. kind filtering is non-recursive and does not follow final-component symbolic links,
            5. an entry that cannot be represented losslessly as scalar-only `Str` makes the whole call return `Err(IoError)` rather than being omitted, substituted, or exposed in a partial result before kind filtering,
            6. stable controlled fixtures MUST establish exact direct-child membership without duplicate paths; sorted output is not evidence of a point-in-time namespace snapshot,
            7. implementation-certification mutation probes MUST establish that one acquired directory object remains fixed while the source path or its final directory entry is replaced, and that additions, removals, and renames are confined to the permitted observed-candidate envelope,
            8. controlled duplicate-record probes MUST establish exact-native-basename deduplication without normalization, case folding, or merging canonically equivalent spellings,
            9. filtered-list mutation probes MUST establish name-bound classification relative to the acquired directory: replacement before classification uses the replacement's own no-follow kind, disappearance or another classification failure returns whole-call `Err`, classification order is unspecified, and no same-entry identity or atomic multi-entry observation is claimed,
            10. injected acquisition, enumeration, native-name conversion, classification, and close failures MUST establish no exposed partial sequence, exact library-owned body/close error precedence, and terminal retirement of the internal enumeration resource,
            11. retry evidence MUST establish fresh path resolution, acquisition, enumeration, and classification rather than automatic replay or continuation from a prior candidate set,
            12. required v0.1 source fixtures MUST NOT depend on constructing a symbolic link or unrepresentable native name, creating a namespace race, substituting metadata, emitting duplicate host records, or injecting lifecycle faults; those obligations remain normative implementation-certification requirements;
         16. explicit file-access link policy:
            1. every byte/text reader, writer, appender, scoped file form, whole-file helper call, and `copy!` source acquisition supplies `LinkMode::Follow` or `LinkMode::Reject`; omission MUST fail with `CALL_MISSING_ARG`,
            2. representative direct regular-file fixtures exercise successful access under both modes,
            3. `LinkMode::Reject` final-entry race safety, final symbolic-link rejection, and `LinkMode::Follow` dangling-link creation behavior remain normative implementation obligations,
            4. required v0.1 source fixtures MUST NOT depend on constructing symbolic links because symbolic-link creation and host-fixture injection are not baseline surfaces; passing only direct-file fixtures MUST NOT be cited as evidence for the final-link runtime obligations;
         17. explicit directory-creation traversal policy:
            1. every `create_dir!` and `create_dir_all!` call supplies `DirTraversal::FollowLinks` or `DirTraversal::RejectLinks`; omission MUST fail with `CALL_MISSING_ARG`,
            2. representative direct-directory fixtures exercise successful creation under both policies,
            3. `DirTraversal::RejectLinks` all-component race safety and rejection, plus `DirTraversal::FollowLinks` traversal and final-link idempotence, remain normative implementation obligations,
            4. required v0.1 source fixtures MUST NOT depend on constructing symbolic links because symbolic-link creation and host-fixture injection are not baseline surfaces; passing only direct-directory fixtures MUST NOT be cited as evidence for symbolic-link traversal behavior;
         18. regular-file acquisition failure-state certification requires:
            1. controlled `open_writer!` and `open_text_writer!` failures before
               and after create-or-truncate commit MUST accept only unchanged,
               absent, newly created empty, or same-object truncated-empty states
               permitted by the selected starting target and `LinkMode`,
            2. controlled `open_appender!` and `open_text_appender!` failures
               before and after create-or-append commit MUST accept only an
               unchanged or absent starting path, or a newly created empty
               regular file; acquisition itself MUST NOT change existing data
               bytes,
            3. controlled reader acquisition failure MUST reject any
               operation-induced create, truncate, or target-data write,
            4. the corresponding scoped form MUST permit the same file states
               when acquisition fails before callback entry,
            5. every post-acquisition failure MUST verify terminal retirement of
               the unpublished host resource, no compensating rollback or
               auxiliary entry, and fresh path resolution on caller retry,
            6. required v0.1 source fixtures MUST NOT require acquisition fault
               injection; deterministic missing-target rejection and successful
               open/close fixtures MUST NOT be cited as evidence for late
               acquisition state or unpublished-resource retirement;
      7. time baseline probes MUST include representative coverage for:
         1. `Duration.nanos` / `Duration.millis` / `Duration.seconds` `Nat` contextual literal boundary failures (`DOMAIN_INVALID_VALUE`),
         2. `Duration.parse` success and failure paths,
         3. duration arithmetic success/failure in value channel,
         4. `now!` effect-surface use plus pure `Instant.duration_since` success
            and reversed-order `Err(TimeError)`,
         5. zero-duration no-op and positive-duration `sleep!` returning only
            after the requested monotonic duration,
         6. spurious-wakeup/host-interruption resumption and host-suspension
            clock policy remain normative implementation obligations; required
            source fixtures MUST NOT require host fault injection or system
            suspension,
         7. exact sleep overshoot is not a deterministic value and MUST NOT be
            compared for equality across executions or tensor determinism
            profiles;
      8. operational error observation probes MUST require:
         1. exact `PathErrorKind` mapping for empty and NUL-containing strict
            path parse failures,
         2. exact `TimeErrorKind` mapping for every closed baseline time failure
            family,
         3. representative `IoErrorKind` mapping for portable source-created
            failures and exact display of every kind variant,
         4. `TextError::Io` display parity with the wrapped `IoError` and stable
            `TextError::InvalidUtf8` display,
         5. injected implementation-certification evidence for portable host
            mappings that cannot be produced reliably by source fixtures,
         6. assertions that kind inspection does not strengthen progress,
            rollback, retry, ownership, or resource-retirement guarantees.
      9. scoped-resource failure-carrier coverage requires:
         1. every byte, text, and standard-stream scoped form to return the
            shared `ScopedFailure` carrier specified by the
            [scoped-resource convention](../stdlib/conventions.md),
         2. portable source fixtures for callback success, callback failure,
            acquisition failure, and fatal runtime termination; fatal runtime
            termination MUST remain on the runtime-error channel rather than be
            wrapped as a typed failure,
         3. when `scoped_close_failure_probe` is active, exact close-only
            `Primary` and callback-plus-close `PrimaryAndClose` values, including
            primary-before-close payload order and exact display,
         4. implementation-certification fault injection across byte and text
            readers, writers, appenders, and standard-stream wrappers; evidence
            MAY be reused only when the implementation documents a shared
            adapter and lifecycle path,
         5. no inference from a source-level carrier fixture to terminal handle
            retirement; the corresponding injected close-lifecycle obligations
            above remain independently required,
         6. ordinary `E` and `ScopedFailure[E]` remain distinct propagation
            types, and nested scoped failures are handled at the inner boundary
            rather than implicitly flattened,
         7. source and catalog signature coverage MUST preserve the authority
            boundary: APIs with `callback[scoped-resource]` return
            `ScopedFailure`, while `read_bytes!`, `read_text!`, `read_lines!`,
            `write_bytes!`, `write_text!`, `write_lines!`, `list_paths!`,
            `list_files!`, `list_dirs!`, `print!`, and `eprint!` retain their raw
            domain-error results under the library-owned managed-resource
            convention.
   2. Operational error kinds are value-channel API categories, not runtime
      diagnostic codes. v0.1 baseline does not require fine-grained runtime
      diagnostic-code families for io/time failures beyond existing global
      code-channel policy.
35. Fine-grained io/time runtime diagnostic-code families (for example `IO_*`, `TIME_*`) are future-extension scope and excluded from required v0.1 baseline conformance until standardized.
36. `std::concurrency` baseline suites require:
   1. `spawn!` trailing-thunk sugar and non-bang misuse paths in compile channel (`EFFECT_VIOLATION`),
      1. representative suites using `spawn! { ... }` MUST exercise zero-argument child-thunk typing and effectful child bodies,
   2. deterministic `par_map!` output ordering independent of callback completion order,
   3. `par_traverse_res!` earliest failing-index semantics:
      1. primary value result is `Err` from the minimal failing input index,
      2. required diagnostic/index assertions select that same minimal input index rather than first-completed failure,
   4. `par_traverse_opt!` `None` propagation through value channel,
      1. `TRAVERSE_OPT_NONE` diagnostics remain `optional_emit_if_present` and are not baseline-mandatory here,
   5. invalid `par_*` workers domain paths:
      1. zero and negative worker counts fail at the `PositiveNat` conversion/contextual-literal boundary before callback scheduling,
      2. representative baseline suite coverage MUST assert `DOMAIN_INVALID_VALUE` with required `domain` and `value` data keys,
      3. baseline suites MUST NOT require a runtime-abort invalid-worker source such as `std::concurrency::par_workers`,
   6. structured scope-exit wait behavior for all child tasks at bang-body lexical exit, including boundary-local progress for one or multiple continuously runnable wait-required children,
   7. explicit `join!` success path with `Res` value-channel result behavior, including correctness for implementations that defer child start until the completion-wait boundary,
   8. explicit `join!` non-bang misuse path in compile channel (`EFFECT_VIOLATION`),
   9. repeated borrowed `join!` through an original task value and a copy returns the same completion result without duplicating execution,
   10. explicit `join!` failure path for a child runtime-fatal outcome:
      1. representative baseline suite coverage MUST assert that explicit `join!` returns through value channel `Err(JoinError)` rather than terminating runtime,
      2. baseline suites MUST NOT require stable public `JoinError` variant/payload inspection,
      3. observing the error through any task-value copy prevents scope-exit re-reporting of that child failure,
      4. when the `ffi_linkage_probe` precondition is active, a runtime
         `EXTERN_LINKAGE_FAILED` in the child is covered by the same `JoinError`
         projection,
   11. unobserved child-failure path at scope exit:
      1. representative baseline suite coverage MUST assert runtime-abort channel `RUNTIME_ABORT`,
      2. `data.source` exact value MUST be asserted as `std::concurrency::scope_exit_join`,
      3. a child `runtime_error` with another inner cause code, including runtime
         `EXTERN_LINKAGE_FAILED`, is projected through this same scope-exit
         result rather than re-emitted directly; the extern case is required
         only when the `ffi_linkage_probe` precondition is active,
   12. portable baseline suites MUST NOT require a particular callback start/completion/effect interleaving for `par_*`; in particular, `workers > 1` does not by itself establish that a later callback began before a decisive result,
   13. fail-fast `par_traverse_*` return-boundary coverage requires:
      1. no unstarted callback above the decisive index begins afterward,
      2. every already-begun callback completes before typed return,
      3. callback effects and cleanup do not cross typed return,
      4. a runtime-fatal outcome from an already-begun callback prevents the otherwise selected typed result,
      5. the conditionally required `par_traverse_quiescence_probe` suite asserts these obligations only under its scheduler-control precondition,
   14. blocked-child and divergent-child controls MUST be compile-checked but skipped by the baseline runner; out-of-band watchdog execution remains harness calibration rather than a language outcome,
   15. baseline required suites MUST NOT require eager spawn start, preemption, a wall-clock completion bound, or parallel speedup; outside an active completion-wait boundary they also MUST NOT require general fairness, starvation-freedom, or eventual-start guarantees,
   16. channel/mutex/atomic coordination primitives are non-baseline in v0.1 and MUST NOT be required by baseline required suites.
37. Unsafe/FFI baseline suites require:
   1. parser acceptance of `extern "<abi>" fn ...` declarations and `unsafe { ... }` expression blocks,
   2. compile-channel enforcement that unsafe operations outside `unsafe { ... }` fail with `UNSAFE_REQUIRED`,
   3. `pub extern ...` declarations fail with `EXTERN_VISIBILITY_FORBIDDEN`,
   4. non-bang extern-call paths (including unsafe-wrapped call sites) fail with `EFFECT_VIOLATION`,
   5. unsafe-wrapped extern call inside bang context is accepted,
   6. omitted extern return types fail with `FN_RETURN_TYPE_REQUIRED`,
   7. unsupported extern signature types fail with `EXTERN_SIGNATURE_UNSUPPORTED`,
      1. representative coverage includes reference-type rejection,
      2. representative coverage includes user-defined nominal ADT/struct signature rejection,
      3. representative coverage includes pointers to user-defined nominal types,
      4. representative coverage includes built-in residual/ownership/layout wrappers (`Opt`, `Res`, `Own`, `Box`),
      5. representative coverage includes `Never`, function, trait-object, and
         tuple type positions,
      6. varargs are not a parse-valid v0.1 `ExternDecl` parameter-list surface and are therefore not asserted through `EXTERN_SIGNATURE_UNSUPPORTED` fixtures,
   8. direct `Bool` parameter and return positions are accepted under the documented C `_Bool` value mapping,
   9. `Unit` is accepted as a direct return type and rejected as a parameter type with `EXTERN_SIGNATURE_UNSUPPORTED`,
   10. pointers whose pointee is `Bool` or `Unit` fail with `EXTERN_SIGNATURE_UNSUPPORTED` in representative parameter and return positions,
   11. unsupported extern ABI selectors fail with `EXTERN_ABI_UNSUPPORTED`,
   12. source raw-pointer types in non-extern positions, including an explicit local annotation inside `unsafe`, fail with `RAW_POINTER_TYPE_UNSUPPORTED`,
   13. confined pointer relay success covers:
      1. direct extern-to-extern chaining and unannotated immutable local aliases,
      2. repeated reads plus `_ = expr`, `let _ = expr`, and expression-statement discards,
      3. same-region `if`/`match` propagation and a nested `unsafe` that preserves the outer relay region,
      4. one-way `*mut T` to extern `*const T` argument weakening,
   14. `RAW_POINTER_USE_UNSUPPORTED` covers representative region/callable results, mutable and aggregate storage, inferred generic instantiation, ordinary non-extern calls, capture, comparison, and dereference paths:
      1. each fixture asserts required `use` and `type` data keys,
      2. each fixture with an exact `use` assertion uses the stable values in `compile-codes.md`,
      3. these failures take precedence over the ordinary return, generic, capture, comparison, and operator diagnostics for the same use,
   15. an otherwise permitted extern argument that would require `*const T` to `*mut T` conversion or a different pointee type fails with `CALL_ARG_TYPE_MISMATCH`,
   16. pointer-producing `if`/`match` branches without one exact raw-pointer type fail with `IF_BRANCH_TYPE_MISMATCH` / `MATCH_ARM_TYPE_MISMATCH`; extern-argument weakening does not supply a branch common type,
   17. baseline suites MUST NOT infer pointer lifetime, provenance, nullability, deallocation, exclusivity, or pointee validity from successful relay compilation,
   18. linkage-failure channel split is honored when exercised:
      1. cause code identity is `EXTERN_LINKAGE_FAILED`,
      2. statically decidable linkage failures surface as `compile_error`,
      3. non-statically-decidable linkage failures surface as `runtime_error`.
      4. Required v0.1 baseline suites require only the portable non-statically-decidable runtime probe. A deterministic compile-time linkage-failure fixture is not required in v0.1 because the baseline extern surface has no portable link-library/path declaration or standardized static symbol table input. Implementations that do statically decide linkage failure MUST still use `EXTERN_LINKAGE_FAILED` in `compile_error`.
   19. dynamic-link effect enforcement in v0.1 baseline is validated through compile-channel effect classification (`EFFECT_VIOLATION`) and does not require runtime side-effect observation probes.
38. `std::parse` baseline suites require:
   1. representative success/failure value-channel coverage for full scalar parse surface:
      1. signed integers: `parse_i8`, `parse_i16`, `parse_i32`, `parse_i64`,
      2. unsigned integers: `parse_u8`, `parse_u16`, `parse_u32`, `parse_u64`,
      3. alias parity: `parse_int` behavior parity with `parse_i64`,
      4. floating: `parse_f16`, `parse_bf16`, `parse_f32`, `parse_f64`,
      5. complex: `parse_complex64`, `parse_complex128`,
      6. boolean: `parse_bool`;
   2. integer parser coverage includes:
      1. optional sign acceptance for signed parsers,
      2. explicit sign rejection for unsigned parsers,
      3. out-of-range rejection and malformed-grammar rejection;
   3. floating parser coverage includes:
      1. decimal and/or exponent forms,
      2. case-insensitive special-token acceptance for `nan`/`inf` with optional sign,
      3. malformed-grammar rejection;
   4. complex parser coverage includes:
      1. real-only, imaginary-only, cartesian-token, and tuple forms,
      2. inherited `nan`/`inf` component token acceptance,
      3. malformed-grammar rejection;
   5. whitespace policy coverage includes at least one parser case showing leading/trailing whitespace rejection in v0.1 baseline;
   6. invalid parse inputs in baseline are asserted through `expect.value.kind`/`Res::Err` behavior (not `runtime_error`) under global code-channel policy.
39. `std::map` / `std::set` baseline suites require:
   1. required query-surface coverage for constructors and pure queries:
      1. `Map`: `new`, `len`, `is_empty`, `contains_key`, `get`, `keys`, `values`, `items`,
      2. `Set`: `new`, `len`, `contains`, `items`;
   2. consuming update coverage:
      1. `Map.insert` / `Map.remove` preserve value semantics of source bindings,
      2. `Set.insert` / `Set.remove` preserve value semantics of source bindings;
   3. in-place update return-contract coverage:
      1. `Map.insert!` / `Map.remove!` previous-value `Opt` behavior,
      2. `Set.insert!` / `Set.remove!` boolean success-flag behavior;
   4. deterministic insertion-order iteration observables remain asserted for map/set query outputs;
   5. type-head key capability coverage requires:
      1. successful `Map`/`Set` formation and lookup with a user type implementing both `Eq` and `Hash`,
      2. `TRAIT_BOUND_NOT_SATISFIED` when either `Eq` or `Hash` is absent,
      3. successful key use for required structural `Hash` families;
   6. floating-key coverage requires distinct `-0.0` and `+0.0` keys and successful lookup of an inserted NaN key under the unified total-key identity.
40. `std::net` baseline suites require:
   1. representative pure parse coverage for `parse_net_addr`:
      1. valid IPv4 `host:port` parse success path,
      2. malformed/out-of-range parse failure path in value channel;
   2. representative effect-boundary coverage:
      1. non-bang call sites invoking `tcp_connect!` or `tcp_bind!` fail in compile channel (`EFFECT_VIOLATION`);
   3. representative runtime value-channel coverage on loopback addresses:
      1. `tcp_bind!` success path with port `0`, `Own[TcpListener]` metadata access, and consuming close,
      2. `tcp_connect!` failure path asserted through `expect.value.kind`/`Res::Err` (not `runtime_error`),
      3. local loopback connect/accept path with `with_tcp_listener!`, `with_tcp_stream!`, and `accept_with!` scoped cleanup,
      4. `Own[TcpStream].write_all!` / `Own[TcpStream].read_chunk!` loopback byte-transfer success path,
      5. `Own[TcpStream].read_chunk!` EOF returns `Ok(None)` and invalid `max_bytes` domain paths use `DOMAIN_INVALID_VALUE`,
      6. repeated close, post-close I/O, and post-close metadata source use fail through ownership diagnostics;
      7. loopback transfer success does not establish `TcpStream.read_chunk!`
         failure progress; fault-injected evidence MUST verify ordered delivery
         of bytes received before a later transport failure, zero logical byte
         progress on outward `Err`, no skip or duplication when a later read
         succeeds, and continued availability of the stream root for close.
         Required v0.1 source fixtures MUST NOT require a controllable transport
         failure;
      8. loopback transfer success does not establish `TcpStream.write_all!`
         failure progress or peer observation; fault-injected short-write or
         late-failure evidence remains an implementation-certification
         obligation, and required v0.1 source fixtures MUST NOT require a
         controllable transport failure;
      9. acquisition failure certification requires controlled failures after
         bind, connect, and accept transport progress:
         1. `tcp_bind!` and `tcp_connect!` `Err` MUST return no source root and
            MUST terminally retire each provisional local endpoint before return,
         2. `tcp_connect!` evidence MAY observe a connection attempt, completed
            handshake, or peer EOF/reset, but MUST verify that acquisition sends
            no application payload,
         3. `accept!` `Err` MUST leave the listener root usable, MAY consume one
            pending connection, and MUST terminally retire any unpublished
            accepted endpoint; a later accept is a fresh queue operation,
         4. the same acquisition effects apply when the corresponding scoped form
            fails before callback entry,
         5. ordinary connection-refusal and successful loopback fixtures MUST NOT
            be cited as evidence for post-progress acquisition cleanup;
      10. network close certification requires target-adapter fault injection and
          local-handle liveness evidence:
         1. listener close `Ok` and `Err` MUST terminally retire the listener
            without invalidating already accepted stream roots,
         2. stream close injection before, during, and after pending output
            transfer MUST verify terminal local retirement on either result and
            MUST accept only ordered peer payload prefixes while permitting EOF,
            reset, or no terminal peer observation,
         3. successful loopback EOF fixtures establish the ordinary orderly-close
            path only and MUST NOT be cited as evidence for peer receipt, close
            `Err` observations, or terminal handle retirement after host failure,
         4. instrumentation MUST reject retained usable handles,
            implementation-managed hidden cleanup after return, automatic
            source-level close replay, and unsafe retries against reused host
            resource identities;
      11. network scoped-resource failure-carrier coverage requires:
         1. `with_tcp_listener!`, `with_tcp_stream!`, and `accept_with!` to
            return the shared `ScopedFailure[NetError]` carrier,
         2. controlled close-only and callback-plus-close failures to preserve
            exact `Primary` and primary-before-close `PrimaryAndClose` values,
         3. close-adapter evidence to be reused only when the implementation
            documents the same adapter and lifecycle path,
         4. fatal runtime termination to remain on the runtime-error channel and
            never be wrapped as `ScopedFailure`;
   4. signature-surface coverage for stream/listener methods:
      1. `Own[TcpListener].local_addr` / `Own[TcpListener].accept!` / `Own[TcpListener].accept_with!` / consuming `Own[TcpListener].close!`,
      2. `Own[TcpStream].peer_addr` / `Own[TcpStream].read_chunk!` / `Own[TcpStream].write_all!` / consuming `Own[TcpStream].close!`.
   5. network error observation coverage requires:
      1. exact `InvalidAddress` kind and display for malformed address input,
      2. exact display of every `NetErrorKind` variant,
      3. injected implementation-certification evidence for host-network cause
         mappings that cannot be produced reliably by portable source fixtures,
      4. assertions that a kind does not strengthen connection progress, byte
         progress, peer observation, retry, ownership, or endpoint-retirement
         guarantees.
41. `std::json` baseline suites require:
   1. representative pure parse coverage for:
      1. object/array/scalar success paths,
      2. malformed grammar rejection (for example trailing comma) in value channel,
      3. duplicate-key rejection in `parse_json`,
      4. inexact finite number conversion using IEEE-754 binary64 `roundTiesToEven`,
      5. largest-finite and subnormal number acceptance,
      6. signed-zero preservation for exact zero and underflow to zero,
      7. number overflow rejection when conversion would produce positive or negative infinity;
   2. representative deterministic serialization coverage for:
      1. compact output (no insignificant whitespace),
      2. stable object-member order preservation,
      3. canonical number rendering and canonical string escape normalization;
   3. representative constructor/projection coverage for:
      1. `json_object` success with entry-order preservation,
      2. duplicate-key rejection in `json_object`,
      3. non-finite-number rejection in `json_number`,
      4. exact-kind `as_*` projection behavior (`Some` only for matching JSON kind);
   4. baseline json failures are asserted through `expect.value.kind`/`Res::Err` (not `runtime_error`).
42. Control-flow baseline suites require:
   1. `if` validation paths require:
      1. `IF_CONDITION_NOT_BOOL`,
      2. `IF_BOOL_UNKNOWN_BRANCH_FOR_BOOL`,
      3. `IF_TRI_UNKNOWN_BRANCH_REQUIRED`,
      4. `IF_BRANCH_TYPE_MISMATCH`,
      5. `IF_MISSING_ELSE_VALUE`;
   2. scalar `Tri` logic coverage requires:
      1. helper conversions and pure K3 truth tables,
      2. mixed `Bool`/`Tri` method overloads and `&&` / `||` operator typing, with `Bool` operands lifted to `Tri`;
   3. `match` validation paths require:
      1. `MATCH_NON_EXHAUSTIVE`,
      2. `MATCH_GUARD_EFFECT`,
      3. `MATCH_GUARD_TYPE_MISMATCH`,
      4. `MATCH_UNREACHABLE_ARM`,
      5. `MATCH_BINDING_MISMATCH`, including missing, extra, and differently
         typed or-pattern bindings,
      6. `MATCH_ARM_TYPE_MISMATCH`,
      7. normal-result joins that ignore arms with no normal continuation and
         produce `Never` when no arm has a normal result edge.
   4. loop/range validation paths require:
      1. `RANGE_EXPR_UNSUPPORTED` for range syntax outside a `for-in` iterable expression.
   5. loop-control validation paths require:
      1. `BREAK_OUTSIDE_LOOP`,
      2. `CONTINUE_OUTSIDE_LOOP`,
      3. representative closure-boundary probes showing closure-local `break` / `continue` cannot target an outer loop.
   6. control-head parser boundary coverage requires:
      1. ordinary body braces remain valid for `if`, `while`, `for`, and `match`,
      2. a brace-led expression may begin a control head,
      3. struct literals, named-payload variants, and trailing closures remain
         available inside an explicit delimiter,
      4. attempts to consume an outer-depth control-body brace as one of those
         ordinary expression forms fail with
         `PARSE_CONTROL_HEAD_BRACE_REQUIRES_GROUPING`.
43. Trait baseline suites require:
   1. `TRAIT_STATE_FORBIDDEN` for trait state declarations;
   2. `TRAIT_IMPL_MISSING_METHOD` for omitted non-default trait methods;
   3. `TRAIT_IMPL_SIGNATURE_MISMATCH` for signature mismatches other than bang-marker mismatch, including associated-type-normalized results and method generic-bound-set differences; alpha-renamed, reordered equivalent bound sets remain accepted;
   4. `TRAIT_EFFECT_MISMATCH` for trait/impl bang-marker mismatch;
   5. `TRAIT_METHOD_AMBIGUOUS` for ambiguous visible trait-method dispatch from
      default or impl-provided method bodies;
   6. `TRAIT_BOUND_INVALID` for a non-trait or duplicate inline bound;
   7. `TRAIT_BOUND_NOT_SATISFIED` for inferred calls, explicit calls, expected-`Fn` instantiations, inferred generic nominal constructors, non-call generic type applications, generic trait references, generic impl obligations, and unique structurally matched method impls that fail a declared bound, including exact symbolic trait-argument mismatch, leftmost-primary multi-bound failure, a recursive obligation that cannot prove itself, and a recursive bound encountered while selecting an associated result;
   8. `TRAIT_IMPL_ASSOC_TYPE_MISMATCH` for missing, unknown, or duplicate associated-type impl bindings, including precedence over an invalid supplied RHS when another required binding is missing;
   9. `ASSOCIATED_TYPE_PROJECTION_INVALID` for unbound, unknown, ambiguous, concrete-base, chained, direct or alias-hidden impl-header, qualified concrete-path, and imported concrete-alias projection attempts;
   10. `ASSOCIATED_TYPE_CYCLE` for concrete and generic-symbolic binding cycles;
   11. `CLOSED_TRAIT_IMPL_FORBIDDEN` for an impl outside the closed trait's declaring package;
   12. `DYN_MUST_BE_EXPLICIT` for implicit trait-object type attempts;
   13. `DYN_TRAIT_UNSUPPORTED` for explicit `dyn Trait` in representative non-extern v0.1 baseline type positions;
   14. impl-header legality and coherence require:
       1. `IMPL_GENERIC_ERROR_UNION_UNSUPPORTED` for direct, nested, independently repeated, and transparent-alias-expanded impl parameters below a normalized structural error union, with header legality preceding authority and coherence,
       2. acceptance of a complete-position error parameter and of a source union that normalizes to that parameter,
       3. normalized member-set identity for rigid unions, including source-order, duplicate, and `Never` invariance, while clearly disjoint rigid unions remain disjoint and a complete-position blanket parameter retains ordinary specialization classification,
       4. coherence diagnostics for orphan, duplicate, overlap, specialization, negative impl, and unconstrained impl-parameter paths; orphan coverage distinguishes local and foreign generic target heads, direct and nested local arguments under a foreign head, transparent aliases, reference and structural heads under local and foreign traits, the absence of a `Box[...]` fundamental-wrapper exception, the implementation-owned tensor head, and orphan-before-overlap precedence;
   15. successful paths for inferred and explicit bounded calls, inferred bounded nominal constructors, symbolic generic well-formedness assumptions, non-call bounded type applications, same-carrier returns, argument-driven and symbolic-nominal associated-result normalization, impl-signature normalization, bounded-impl associated bindings, borrows written outside generic parameters and associated projections, imported public associated types, same-package closed-trait impls, and qualified trait methods coerced through an expected `Fn` receiver;
   16. bound-only differences and associated binding/projection differences do not make overload or impl declarations disjoint;
   17. distinct unresolved canonical projections do not become equal from the current impl set;
   18. an out-of-scope `Self::Item` projection, including one in an impl header, preserves `SELF_TYPE_INVALID` precedence;
   19. projected parameter, immediate result annotation, and expected-`Fn` components do not reverse-infer their base type;
   20. `REFERENCE_TYPE_POSITION_FORBIDDEN` for inferred, explicit, expected-`Fn`, and generic-trait-reference instantiations with reference-bearing arguments, including mutable and nested generic forms;
   21. `REFERENCE_TYPE_POSITION_FORBIDDEN` for associated bindings that conceal direct, alias-expanded, or callable-nested references;
   22. diagnostic precedence proves binding completeness before RHS validation, ownership-position failure before reference-free failure, and reference-free failure before trait-bound, substituted `NON_BANG_MUT_PARAM_FORBIDDEN`, or substituted `REF_ESCAPE_FORBIDDEN` failures;
   23. zero-arity associated types reject GAT-like type arguments with `TYPE_ARG_ARITY_MISMATCH`;
   24. alias-expanded projections retain origin provenance through concrete normalization for impl-head rejection and conservative overload overlap;
   25. public-trait/public-carrier conformance families, including intrinsic public target heads, reject unnameable impl generic bounds, other header components, and direct or nested unnameable associated-binding types with `VISIBILITY_VIOLATION`, allow private source aliases whose complete expansion is externally nameable, and permit private traits or private carriers to use private types while public generic conformances retain valid symbolic projections; impl authorization and coherence precede header visibility, reference-free binding validation precedes binding visibility, and inaccessible qualified bases preserve `VISIBILITY_VIOLATION` precedence over projection classification;
   26. associated names are not standalone import symbols and reject direct imports with `IMPORT_NOT_FOUND`;
   27. the contextual `Self` spelling is reserved against nominal, type-alias, and generic-parameter declarations with `RESERVED_BUILTIN_NAME` while remaining valid in the value namespace;
   28. recursive-layout summaries distinguish projection base identity and substitution path so a boxed edge for one base does not merge with another base's inline edge;
   29. the standard tensor capability surface verifies prelude-visible
       `ReadableTensor`, explicitly imported `AffineTensor`, exact associated
       element types, canonical trait-owned query callables, rejected
       concrete-owner callable references, closed-trait impl authority, and
       symbolic calls from explicit `C: ReadableTensor` and
       `C: ReadableTensor + AffineTensor` source parameters into matching
       standard-library APIs.
   30. trait default-body declaration checking requires:
       1. the exact normalized `Self: CurrentTrait[args]` assumption to compose
          through an ordinary generic helper and remain valid when concrete
          dispatch selects the default body,
       2. all trait member signatures to be available before body checking so
          mutual default references terminate without body expansion,
       3. `TRAIT_BOUND_NOT_SATISFIED` at the default declaration for an
          unrelated trait, differently instantiated current trait, or
          recursively derived external obligation rather than deferring those
          requirements to concrete impl selection.

44. Tensor carrier boundary suites require:
   1. `ReadableTensor` alone does not satisfy an affine movement boundary; `AffineTensor` remains an explicit additional bound;
   2. every affine movement primitive and `view{...}` composition accepts `TensorView` directly and returns `TensorView` without implicit materialization;
   3. independently generic readable operand positions accept `TensorView` for elementwise operations, `rearrange`, `contract`/`matmul`, and `einop`, while every materializing result is statically observed as owning `Tensor`;
   4. `astype`, `bitcast`, `wrapping_cast`, `sum`, reduction-form `max`, and `mean` accept `TensorView` directly and return owning `Tensor`;
   5. `get_at` accepts a view source plus view-backed `Take` coordinates and returns owning `Tensor`, while `set_at!` rejects a `TensorView` destination with `CALL_ARG_TYPE_MISMATCH`;
   6. owning name-overlay methods reject `TensorView` receivers with `METHOD_NOT_FOUND` and direct users to explicit materialization;
   7. `grad` and `value_and_grad` preserve a concrete `TensorView` differentiated parameter and return owning gradient/primal values, while rejecting `TensorView` primal-result callback families with `CALL_ARG_TYPE_MISMATCH`;
   8. retained-view value semantics cover source scope exit, source data and metadata mutation, source binding reassignment, copied views, and explicit materialization isolation.

45. Program entrypoint boundary suites require:
   1. executable `runner: main` selects top-level `main!` only from the
      designated entry source, including explicit file-set and package entries
      whose path is not fixed to `src/main.eglur`;
   2. missing, duplicate, parameterized, generic, arbitrary-result, and
      ownership-bearing-result entrypoints fail with
      `PROGRAM_ENTRY_MISSING`, `PROGRAM_ENTRY_DUPLICATE`, or
      `PROGRAM_ENTRY_SIGNATURE_INVALID` as applicable;
   3. `main!() -> Never` is accepted and has no normal process-result projection;
   4. transparent result aliases normalize before entry-result validation;
   5. the harness observes the entrypoint value before process projection:
      1. `Unit` and `Ok(_)` map to `process_exit: 0`,
      2. `Err(_)` and unrecoverable runtime abort map to `process_exit: 1`;
   6. ordinary source failures precede entrypoint validation, while
      `runner: test` remains independent of program-entry discovery;
   7. process projection does not render the returned value or create implicit
      stdout/stderr output.

46. Structural pattern suites require:
   1. exact and tail-rest tuple arity coverage, including rest absorbing zero
      elements;
   2. declaration-form matching for unit, zero-arity positional, positional,
      and named-payload variants;
   3. exact-by-default named fields and explicit `..` omission for structs and
      named-payload variants;
   4. recursive validation of nested tuple, struct, variant, alias, and
      or-pattern nodes;
   5. direct assertions for every `PATTERN_*` diagnostic, including both the
      `structural_form` and `typed_pattern` reasons of
      `PATTERN_TYPE_MISMATCH`;
   6. precedence coverage for outer structure before children, duplicate before
      unknown before missing fields, and structural validity before binding and
      coverage analysis;
   7. contextual literal cause precedence over `PATTERN_TYPE_MISMATCH`, plus
      rejection of interpolated-string pattern syntax;
   8. parser rejection of duplicate `..` in named-payload variant patterns.

47. Parser resource suites require:
   1. the last in-budget prefix-unary chain remains parse-valid;
   2. the next recursive grammar descent fails with
      `PARSE_RESOURCE_LIMIT_EXCEEDED`;
   3. the diagnostic asserts exact `resource=recursive_depth`, `limit=256`,
      and `observed_at_least=257` data;
   4. required source fixtures assert only the primary resource contract and
      MUST NOT be cited as evidence for implementation-specific delimiter/event
      limits, recovery breadth, or wall-clock performance.

If an implementation emits richer diagnostics, it MUST still include these required tokens.
