# Required Compile-time Error Codes

Scope: Normative for v0.1 unless this file or a section states otherwise.

This section defines REQUIRED compile-channel codes currently used by conformance tests.

`diagnostic-contracts.yaml` is normative for severity, message, and `data`
fields. The per-code field lists below are explanatory summaries; trigger and
channel rules remain normative prose.

Channel note:

1. `compile_error` assertions are not limited to this file.
2. Cause codes defined in `runtime-codes.md` MAY also appear in `compile_error` when statically decidable, as required by `code-channels.md`.
3. Extension-profile compile codes are defined in `extension-codes.md` and are excluded from required baseline v0.1 coverage.

### `EFFECT_VIOLATION`

Severity: `error`

Trigger:

1. Non-bang function/method calls a bang function/method outside the local-state-only exception defined in `../language/effects.md`.
2. Non-bang function performs direct effectful primitive operation.
3. Non-bang function/method attempts non-local or escaping mutable-state mutation.
4. Effectful closure, bang named function, or bang-target partial application is supplied where a pure `Fn(...) -> ...` callable value is required, outside a pure-combinator contract.
5. A non-bang call requires an effect summary that is unavailable or `Unknown`.

Message contract summary:

1. One of `non-bang`, `Fn`, or `pure`.
2. Effect source token (for example `print!`, `now!`, `spawn!`, assignment target path).

Data contract summary (if `data` is present):

1. `caller`.
2. `callee`.


### `EFFECT_SIGNATURE`

Severity: `error`

Trigger:

1. Non-bang method declares `&mut Self` receiver.
2. Bang mutating method does not use `&mut Self` receiver.

Message contract summary:

1. Method name.
2. `&mut Self`.


### `UNSAFE_REQUIRED`

Severity: `error`

Trigger:

1. Unsafe operation site (baseline: call to `extern "<abi>" fn ...`) appears outside an active lexical `unsafe { ... }` boundary.

Message contract summary:

1. `unsafe`.
2. `required`.
3. Unsafe operation symbol when available (for example extern callee name).

Data contract summary (if `data` is present):

1. `operation`.
2. `boundary`.


### `EXTERN_VISIBILITY_FORBIDDEN`

Severity: `error`

Trigger: extern declaration is marked `pub` in v0.1 baseline.

Message contract summary:

1. `extern`.
2. `visibility`.
3. `forbidden`.
4. `pub`.


### `EXTERN_SIGNATURE_UNSUPPORTED`

Severity: `error`

Trigger: extern declaration parameter or return type is outside v0.1 extern signature subset.

Representative unsupported categories include:

1. references (`&T`, `&mut T`),
2. user-defined nominal types (`struct`/`enum`) and pointers to them,
3. residual/ownership/layout wrappers (`Opt[...]`, `Res[...]`, `Own[...]`, `Box[...]`),
4. other types outside `../language/unsafe-ffi.md` v0.1 ABI subset.

Message contract summary:

1. `extern`.
2. `signature`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.


### `RAW_POINTER_TYPE_UNSUPPORTED`

Severity: `error`

Trigger: raw pointer type (`*const T` or `*mut T`) appears in a non-extern type position in v0.1 baseline.

Allowed v0.1 raw-pointer type positions are limited to extern declaration parameter and return types that satisfy the ABI subset in `../language/unsafe-ffi.md`.

Message contract summary:

1. `raw pointer`.
2. `extern`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.

### `RAW_POINTER_USE_UNSUPPORTED`

Severity: `error`

Trigger: an otherwise valid compiler-inferred raw-pointer relay value is used
outside the closed flow set in `../language/unsafe-ffi.md`.

Representative rejected uses include leaving the outer relay region, returning
from a callable, mutable or aggregate storage, generic instantiation, ordinary
calls, closure/task capture, comparison or pattern use, and pointer access or
conversion operations.

The primary span MUST identify the first disallowed use of the relay value.

Message contract summary:

1. `raw pointer`.
2. `use`.
3. `unsupported`.

Required data contract:

1. `use`.
2. `type`.

The exact `use` value MUST be one of:

1. `region_escape`,
2. `callable_escape`,
3. `mutable_storage`,
4. `aggregate_storage`,
5. `generic_transport`,
6. `non_extern_call`,
7. `capture`,
8. `operation`.

These values classify forbidden value flow rather than duplicating expression
grammar:

1. `region_escape` means that the pointer is carried across the result boundary of the outermost relay-region `unsafe` expression.
2. `callable_escape` means that an explicit callable result carries the pointer directly out of the active relay region.
3. `mutable_storage` means that the pointer is written to a `let mut` binding, assignment destination, or other mutable storage.
4. `aggregate_storage` means that the pointer enters a tuple, array, struct, enum, residual, collection, or other aggregate.
5. `generic_transport` means that explicit or inferred generic instantiation would carry the pointer.
6. `non_extern_call` means that an ordinary function, method, callback, or partial-call construction would carry the pointer.
7. `capture` means that a closure or child task would capture the pointer.
8. `operation` covers every other value-consuming use, including comparison, hashing, pattern or condition use, iteration, dereference, arithmetic, address-of, conversion, member access, indexing, interpolation, and callee use.

Classification occurs at the first disallowed flow. `region_escape` is selected
before any enclosing destination. `aggregate_storage` is selected when a
generic constructor inserts the pointer into an aggregate and before the
aggregate's containing mutable destination. `generic_transport` is selected for
other generic flow before `non_extern_call`. The compiler MAY retain a more
detailed internal reason, but no additional source-syntax-specific `use` value
is part of the v0.1 diagnostic contract.

### `EXTERN_ABI_UNSUPPORTED`

Severity: `error`

Trigger: extern declaration ABI selector is outside v0.1 baseline subset.

Message contract summary:

1. `extern`.
2. `abi`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `abi`.


### `NON_BANG_MUT_PARAM_FORBIDDEN`

Severity: `error`

Trigger: non-bang function or method declares non-receiver parameter of type `&mut T`.

Message contract summary:

1. `non-bang`.
2. `&mut`.
3. `forbidden`.


### `REF_ESCAPE_FORBIDDEN`

Severity: `error`

Trigger: explicit reference value (`&T` or `&mut T`) escapes its permitted local
storage extent in v0.1. This includes return, escaping capture including capture
by an escaping `Fn` value, storage into an escaping location, or initialization,
assignment, tuple propagation, or control-flow merge into local storage whose
lexical extent can outlive any possible referent.

The primary span MUST identify the first return, capture, store, initialization,
assignment, or merge that crosses the permitted extent in source order.

Message contract summary:

1. `reference`.
2. `escape`.
3. `forbidden`.


### `REFERENCE_TARGET_INVALID`

Severity: `error`

Trigger: the operand of `&` is not a readable stable local place, or the operand
of `&mut` is not a writable stable local place, under
`calls-references.md`. Value-only expressions such as literals, call results,
constructor temporaries, arithmetic results, properties, and module/const
bindings use this code.

Name resolution and operand typing precede reference-target classification.
Reference-target classification precedes `REF_ESCAPE_FORBIDDEN` for the same
reference expression.
The primary span MUST identify the invalid operand.

Message contract summary:

1. `reference`.
2. `target`.
3. `invalid`.

Data contract summary (if `data` is present):

1. `access`.
2. `target_kind`.


### `MUTABLE_CAPTURE_FORBIDDEN`

Severity: `error`

Trigger:

1. A closure literal captures a `let mut` binding, including read-only use.
2. A closure literal assigns to a captured binding or mutates storage rooted in
   a captured binding.

The primary span MUST identify the first offending captured identifier use or
captured-root assignment target in source order.
This code takes precedence over `IMMUTABLE_BINDING_ASSIGN` when direct
assignment targets a captured immutable binding.

Message contract summary:

1. `mutable`.
2. `capture`.
3. `forbidden`.


### `MEMBER_NOT_FOUND`

Severity: `error`

Trigger: a value-member read or stored-field assignment selects a name that
the receiver's static type does not expose as a stored field or property.
This code does not apply to method calls or owner-qualified callable references.

The primary span MUST identify the selected member-name token. Structured data
MUST include `member` and `receiver_type`. The latter identifies the canonical
type whose fields and properties are queried, with transparent aliases expanded.
For a receiver of type `&T` or `&mut T`, it reports `T`.
Missing compiler input is not proof that a member is absent.

Message contract summary:

1. `member`.
2. `not found`.


### `METHOD_NOT_FOUND`

Severity: `error`

Trigger: unresolved method call on a known receiver type, or unresolved
owner-qualified method selection/callable reference on a known type or trait
owner.

Message contract summary:

1. Missing method name.
2. Preferred or nearby candidate when available (for example suggest `sort!` for missing `sort`).


### `METHOD_RECEIVER_INVALID`

Severity: `error`

Trigger: a struct-body, trait-body, or trait-impl callable declaration has no valid first receiver parameter in v0.1.

Invalid receiver shapes include missing receiver parameters, receiver parameters not in first position, receiver names other than `self`, duplicate `self` parameters, and receiver types outside `Self`, `&Self`, or `&mut Self`.

Message contract summary:

1. `method`.
2. `receiver`.
3. `invalid`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.


### `SELF_TYPE_INVALID`

Severity: `error`

Trigger: the contextual `Self` type appears outside a struct-body method, trait method, trait-impl method, or associated-type binding RHS where v0.1 defines it. The exact spelling `Self` is classified before ordinary type-name lookup; a user declaration that attempts to introduce that spelling instead uses `RESERVED_BUILTIN_NAME`.

Message contract summary:

1. `Self`.
2. `scope`.
3. `invalid`.

Data contract summary (if `data` is present):

1. `position`.


### `CALL_POSITIONAL_AFTER_NAMED`

Severity: `error`

Trigger: a call argument list places a positional argument after any named argument.

Message contract summary:

1. `call`.
2. `positional`.
3. `named`.
4. `order`.


### `CALL_DUPLICATE_NAMED_ARG`

Severity: `error`

Trigger: one call binds the same parameter name more than once through named arguments.

Message contract summary:

1. `call`.
2. `named`.
3. `duplicate`.
4. Parameter name when available.

Data contract summary (if `data` is present):

1. `name`.


### `CALL_UNKNOWN_NAMED_ARG`

Severity: `error`

Trigger: a call provides a named argument that does not match any exposed callee parameter name, including any named argument on a `Fn`-typed value call because `Fn` types expose no parameter names.

Message contract summary:

1. `call`.
2. `named`.
3. `unknown`.
4. Argument name when available.

Data contract summary (if `data` is present):

1. `name`.


### `CALL_MISSING_ARG`

Severity: `error`

Trigger: after positional/named argument binding, one required callee parameter has no bound argument.

Message contract summary:

1. `call`.
2. `argument`.
3. `missing`.
4. `parameter`.

Data contract summary (if `data` is present):

1. `parameter`.


### `CALL_TOO_MANY_ARGS`

Severity: `error`

Trigger: a call provides more positional arguments than can be bound to callee parameters after applying v0.1 argument binding rules.

Message contract summary:

1. `call`.
2. `argument`.
3. `too many`.


### `CALL_ARG_TYPE_MISMATCH`

Severity: `error`

Trigger: a bound call argument expression is not assignable to the corresponding callee parameter type, including `Fn(...) -> ...` callable arity, parameter-type, or result-type incompatibility; a generic nominal constructor's complete owner type-argument vector has been fixed by expected context or other valid inference constraints and a field or payload expression is not assignable to the specialized slot; or no free-function or owner-qualified overload candidate exactly matches the independently typed argument vector. This code also covers a dedicated compile-time constructor argument, such as a tensor dtype-bracket type, that is outside the resolved constructor's accepted argument family.

Message contract summary:

1. `call`.
2. `argument`.
3. `type`.
4. `mismatch`.

Data contract summary:

1. `reason`: `parameter_type`, `type_family`, or `overload_no_match`.
2. `actual_type`: the canonical type of the rejected value. For
   `overload_no_match`, this is the canonical tuple type of the independently
   typed argument vector.
3. For `parameter_type`, `parameter` identifies the bound declaration slot and
   `expected_type` is its canonical expected type. `parameter` is the declared
   parameter or field name; an unnamed positional constructor slot is `#N`,
   where `N` is its zero-based declaration position.
4. For `type_family`, `expected_family` identifies the closed accepted family.
   Baseline family identifiers are
   `constructor_dtype`, `tensor_carrier`, and `phase_payload_dtype`.
5. For `overload_no_match`, `candidates` contains the canonical signatures of
   the rejected overload set in declaration-candidate order.

The producer MUST NOT select an arbitrary overload or family member merely to
emit `expected_type`.


### `CALLABLE_VALUE_NO_MATCH`

Severity: `error`

Trigger: an expected `Fn(...) -> ...` type is available for a named overload
set used as a function value, but no candidate is compatible after complete
generic instantiation and directional callable assignability. A single
non-generic function already has a callable type, so its incompatibility uses
the enclosing assignment or call-argument diagnostic. A single generic
function whose type arguments cannot be determined uses
`TYPE_ARG_INFERENCE_FAILED`.

Message contract summary:

1. `callable`.
2. `value`.
3. `no matching candidate`.

Data contract summary:

1. `expected_type`: the canonical expected `Fn` type.
2. `candidates`: the rejected callable signatures in declaration-candidate
   order.

The producer MUST NOT invent an `actual_type` for an unselected overload set.


### `CONSTRUCTOR_TARGET_KIND_MISMATCH`

Severity: `error`

Trigger: a struct literal or variant-constructor head resolves to exactly one
binding, but that binding's normalized semantic kind is not valid for
the written constructor surface. A struct literal requires `struct`; a variant
constructor requires `variant_constructor`.

Message contract summary:

1. `constructor`.
2. `target`.
3. `kind`.
4. `mismatch`.

Data contract summary:

1. `target`: the selected source name, primitive type name, or trusted catalog identity.
2. `expected_kind`: `struct` or `variant_constructor`, according to the written
   surface.
3. `actual_kind`: the selected binding's normalized semantic target kind.

`expected_kind` and `actual_kind` use the closed target-kind vocabulary in
`data-keys.md`. Unresolved, ambiguous, inaccessible, poisoned, or unperformed
lookups retain their earlier diagnostics or capability ownership. If the
declaration graph and canonical signature facts disagree, the implementation
MUST report an internal failure rather than this source diagnostic.

Target-kind validation precedes payload form, fields, payload expressions, and
generic inference. The primary span MUST identify the written constructor head.
When the selected target is a source declaration, its declaration span MUST be
attached as related context.


### `CONSTRUCTOR_PAYLOAD_FORM_MISMATCH`

Severity: `error`

Trigger: after a variant constructor resolves, its written bare,
parenthesized, or braced payload surface differs from the declaration's unit,
positional, or named form. A zero-arity positional variant still requires
parentheses.

Message contract summary:

1. `constructor`.
2. `payload`.
3. `form`.
4. `mismatch`.

Data contract summary:

1. `constructor`: the canonical selected constructor identity. Source variants
   use `Owner::Variant`; trusted declarations use their canonical catalog path.
2. `expected_form`: `unit`, `positional`, or `named`.
3. `actual_form`: `unit`, `positional`, or `named`.

Constructor resolution and `CONSTRUCTOR_TARGET_KIND_MISMATCH` validation
precede this code. This code precedes payload arity, named-field,
payload-expression typing, and generic owner-inference diagnostics.

The primary span MUST identify the constructor head together with its written
payload delimiter when present; a bare form uses the head alone.


### `TUPLE_PROJECTION_NON_TUPLE`

Severity: `error`

Trigger: numeric tuple projection syntax (`expr.0`, `expr.1`, ...) is applied to a receiver whose static type is not a tuple type.

Message contract summary:

1. `tuple`.
2. `projection`.
3. `non-tuple`.

Data contract summary (if `data` is present):

1. `receiver_type`.


### `TUPLE_PROJECTION_OUT_OF_BOUNDS`

Severity: `error`

Trigger: numeric tuple projection syntax uses an index greater than or equal to the static tuple arity.

Message contract summary:

1. `tuple`.
2. `projection`.
3. `out of bounds`.

Data contract summary (if `data` is present):

1. `index`.
2. `arity`.


### `OVERLOAD_MARKER_REQUIRED`

Severity: `error`

Trigger: same-name callable declarations exist in one owner scope, but at least one declaration omits explicit `overload`.

Message contract summary:

1. `overload`.
2. `required`.
3. Callable name.


### `OVERLOAD_VISIBILITY_MISMATCH`

Severity: `error`

Trigger: after every same-name declaration in a module-level or inherent overload set has explicit `overload`, the set contains both public and private declarations.

Message contract summary:

1. `overload`.
2. `visibility`.
3. `mismatch`.
4. Callable name.

Primary-span requirement:

1. Select the first declaration in source order whose visibility differs from the overload set's first declaration.
2. Related information SHOULD point to that first declaration as the visibility established for the family.


### `OVERLOAD_RETURN_ONLY_DIFFERENCE`

Severity: `error`

Trigger: two overload declarations differ only by return type.

Message contract summary:

1. `overload`.
2. `return type`.
3. `forbidden`.


### `OVERLOAD_OVERLAP`

Severity: `error`

Trigger: two overload declarations in one overload set are both applicable to at least one argument-type vector. Alias-expanded or normalized associated projections retain projection-origin provenance for this check and cannot be used to prove declarations disjoint.

Message contract summary:

1. `overload`.
2. `overlap`.
3. Callable name.

Data contract summary (if `data` is present):

1. `name`.
2. `candidates`.


### `OVERLOAD_AMBIGUOUS`

Severity: `error`

Trigger: overload resolution for a call leaves more than one applicable
candidate after v0.1 applicability filtering, or expected-`Fn` coercion of a
named overload set leaves more than one callable-compatible candidate before
checking their declared trait bounds. A bound failure does not remove a
callable-compatible candidate from this ambiguity check.

Message contract summary:

1. `overload`.
2. `ambiguous`.
3. Callable name.

Data contract summary (if `data` is present):

1. `name`.
2. `candidates`.


### `PURE_COMBINATOR_EFFECTFUL_CALLBACK`

Severity: `error`

Trigger: effectful callback provided to a higher-order API parameter whose contract requires a pure callback. The enclosing API may itself be bang for an independent effect such as receiver mutation.

The primary span MUST identify the callback argument expression that supplies
the effectful callable.

Message contract summary:

1. Combinator name (for example `traverse_res`).
2. Phrase `effectful callback`.
3. Suggested bang variant name when available (for example `traverse_res!`).


### `Q_OUTSIDE_RESULT`

Severity: `error`

Trigger: use of `?` without an active `result { ... }` short-circuit context, including inside a `checked { ... }` body whose nearest outer `result` context is blocked by the checked boundary.

Message contract summary:

1. `?`.
2. `result`.


### `RESULT_BLOCK_MIXED_KINDS`

Severity: `error`

Trigger: a single `result` block mixes `Res`-`?` and `Opt`-`?` without explicit conversion.

Message contract summary:

1. `Res`.
2. `Opt`.
3. `mixed`.


### `RESULT_MISSING_YIELD`

Severity: `error`

Trigger: `result` block ends without explicit `yield` in v0.1.

Message contract summary:

1. `yield`.


### `INHERITANCE_FORBIDDEN`

Severity: `error`

Trigger: implementation inheritance syntax is used.

Message contract summary:

1. `inheritance`.
2. `forbidden`.


### `TRAIT_STATE_FORBIDDEN`

Severity: `error`

Trigger: trait declares `field` or `prop` members.

Message contract summary:

1. `trait`.
2. `state`.


### `TRAIT_METHOD_AMBIGUOUS`

Severity: `error`

Trigger: receiver-method dispatch leaves multiple applicable visible trait
methods, whether their bodies come from trait defaults or concrete impl
methods.

Message contract summary:

1. At least two candidate trait-qualified names (for example `A.f`, `B.f`).
2. `fully qualified` guidance.


### `TRAIT_EFFECT_MISMATCH`

Severity: `error`

Trigger: trait method bang marker (`!`) does not match implementation signature.

Message contract summary:

1. Trait method name.
2. Impl method name.
3. `must match`.

Data contract summary:

1. `trait`.
2. `method`.
3. `expected_signature`.
4. `actual_signature`.


### `TRAIT_IMPL_MISSING_METHOD`

Severity: `error`

Trigger: `impl Trait for Type` omits an implementation for a trait method that has no default body.

Message contract summary:

1. `trait`.
2. `method`.
3. `missing`.

Data contract summary (if `data` is present):

1. `trait`.
2. `method`.


### `TRAIT_IMPL_SIGNATURE_MISMATCH`

Severity: `error`

Trigger: an impl method selected for a trait requirement has a signature that does not match the trait method declaration.

Signature mismatches include generic arity, the normalized bound set of each corresponding generic parameter, parameter count, parameter types including receiver type, or return type after accounting for `TRAIT_EFFECT_MISMATCH` on bangness and associated-type normalization.

Message contract summary:

1. `trait`.
2. `impl`.
3. `method`.
4. `signature`.
5. `mismatch`.

Data contract summary:

1. `trait`.
2. `method`.
3. `expected_signature`.
4. `actual_signature`.

Both signatures are rendered after the substitutions and normalization used by
the signature comparison itself.

Recommended related-span guidance:

1. When the trait method declaration is distinct from the failing impl method,
   a `related` span SHOULD identify that declaration.


### `GENERIC_PARAMETER_REDECLARATION`

Severity: `error`

Trigger:

1. A later generic parameter in one `GenericParams` repeats an earlier parameter name.
2. A generic parameter on a nested declaration repeats a generic parameter name from a containing declaration scope.

Message contract summary:

1. `generic parameter`.
2. `redeclare`.
3. Offending parameter name.

Data contract summary:

1. `name`.

Primary-span requirement:

1. The primary span MUST select the later or inner redeclaring parameter.


### `GENERIC_BOUND_REFERENCE_CYCLE`

Severity: `error`

Trigger: completing the normalized declaring trait references for a source generic-bound projection requires a direct or indirect construction cycle under the declaring reference construction rules in `../language/traits.md`. Reuse of a completed reference or ordinary finite mutual generic assumptions is not a cycle. Associated-type binding cycles in an impl, trait proof cycles, and alias expansion cycles retain their existing diagnostics.

Message contract summary:

1. `generic`.
2. `bound`.
3. `reference`.
4. `cycle`.

Data contract summary (if `data` is present):

1. `parameter`.
2. `cycle`.

Primary-span requirement:

1. The primary span MUST identify the earliest source-positioned projection occurrence that contributes a dependency in the selected construction cycle. After transparent alias expansion, attribute the occurrence back to user-written surface syntax.
2. Cycle selection and source-position ordering MUST be independent of resolution traversal order.


### `TRAIT_BOUND_INVALID`

Severity: `error`

Trigger:

1. An inline generic bound does not resolve to a visible trait at the declaration site.
2. One generic parameter repeats the same normalized trait reference in its bound list.

Message contract summary:

1. `trait`.
2. `bound`.
3. `invalid`.

Data contract summary (if `data` is present):

1. `parameter`.
2. `bound`.
3. `reason`.

Primary-span requirement:

1. An unresolved bound selects that bound occurrence.
2. A duplicate bound selects the later repeated occurrence.


### `TRAIT_BOUND_NOT_SATISFIED`

Severity: `error`

Trigger: a complete generic instantiation does not have a unique applicable impl
for a declared trait bound or compiler-known prelude capability prerequisite.
This includes inferred or explicit calls, expected-`Fn` instantiation, generic
type applications, generic trait references, generic impl obligations after
header inference, projection normalization of a substituted nominal base with
symbolic or concrete arguments, an unsatisfied bound on the unique structurally
matching method-dispatch impl, an unsatisfied fixed prerequisite such as
`Ord -> Eq`, and an unresolved recursive trait obligation. An exact obligation
listed in the current declaration's generic well-formedness environment is
satisfied locally. A trait default body's environment additionally contains
only its exact current-trait obligation and fixed compiler-known prerequisites;
a differently instantiated current trait, unrelated trait, or recursively
derived obligation remains unsatisfied under this code.

Message contract summary:

1. `trait`.
2. `bound`.
3. `not satisfied`.
4. Bound trait name.
5. Concrete type when known.

Data contract summary:

1. `trait`.
2. `type`.
3. `parameter` when the failed obligation belongs to a named generic
   parameter.

`trait` and `type` are required. `parameter` is supplemental because fixed
prerequisites, method-dispatch obligations, and recursive obligations need not
belong to one generic parameter.

Primary-span requirement:

1. The primary span MUST identify the application, instantiation, impl
   obligation, dispatch use, or projection whose complete obligation is not
   satisfied.
2. The generic bound declaration MAY be reported as a related span and MUST NOT
   replace a distinct failing use site.


### `TRAIT_IMPL_ASSOC_TYPE_MISMATCH`

Severity: `error`

Trigger: a trait impl has a missing, unknown, or duplicate associated-type binding relative to the implemented trait.

Message contract summary:

1. `trait`.
2. `impl`.
3. `associated type`.
4. `mismatch`.

Data contract summary:

1. `trait`.
2. `associated_type`.
3. `reason`: `missing`, `unknown`, or `duplicate`.

Primary-span requirement:

1. An unknown or duplicate binding selects the offending binding declaration;
   a duplicate selects the later occurrence.
2. A missing binding selects the `impl` header because no binding occurrence
   exists.


### `ASSOCIATED_TYPE_PROJECTION_INVALID`

Severity: `error`

Trigger: an associated-type projection has a disallowed generic or concrete base, appears directly or through a transparent alias in a forbidden impl-header position, contains a chained member after its associated name, has no declaring bound, names an unknown associated type, or has more than one declaring bound for that name. A qualified concrete path such as `dep::api::Count::Item` is a concrete-base projection attempt. An out-of-scope `Self::Name` fails earlier with `SELF_TYPE_INVALID`.

Message contract summary:

1. `associated type`.
2. `projection`.
3. `invalid`.

Data contract summary (if `data` is present):

1. `base`.
2. `name`.
3. `reason`.

Primary-span requirement:

1. The primary span MUST identify the invalid projection occurrence after
   transparent alias expansion is attributed back to user-written surface
   syntax.


### `ASSOCIATED_TYPE_CYCLE`

Severity: `error`

Trigger: associated-type bindings form a direct or indirect cycle while resolving bindings in one impl declaration after `Self` and impl substitutions are applied. This includes a canonical symbolic cycle closed within a generic impl declaration. A recursive trait obligation encountered before a binding cycle is established uses `TRAIT_BOUND_NOT_SATISFIED` instead.

Message contract summary:

1. `associated type`.
2. `cycle`.

Data contract summary (if `data` is present):

1. `trait`.
2. `type`.
3. `cycle`.

Primary-span requirement:

1. The primary span MUST identify the earliest source-positioned
   associated-type binding right-hand side that participates in the selected
   cycle.
2. Cycle selection and source-position ordering MUST be independent of
   resolution traversal order.


### `CLOSED_TRAIT_IMPL_FORBIDDEN`

Severity: `error`

Trigger: a package other than the declaring package attempts to define an impl of a `closed trait`.

Message contract summary:

1. `closed`.
2. `trait`.
3. `impl`.
4. `package`.

Data contract summary (if `data` is present):

1. `trait`.
2. `declaring_package`.
3. `implementing_package`.


### `DYN_MUST_BE_EXPLICIT`

Severity: `error`

Trigger: dynamic trait object usage without explicit `dyn` marker.

Message contract summary:

1. `dyn`.
2. Trait name when available.


### `DYN_TRAIT_UNSUPPORTED`

Severity: `error`

Trigger: explicit `dyn Trait` type surface appears in a non-extern v0.1 baseline type position.

Message contract summary:

1. `dyn`.
2. `trait`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `trait`.


### `UNION_TYPE_ARITY_MISMATCH`

Severity: `error`

Trigger: the reserved source type head `Union` supplies fewer than two type arguments. This well-formedness check precedes union-position and extern-signature validation; `Union[]` remains a parser error because `TypeArgs` is non-empty.

Message contract summary:

1. `union`.
2. `type argument`.
3. `arity`.
4. Minimum and actual argument counts.

Data contract summary:

1. `minimum_count`: `2`.
2. `actual_count`: the number of supplied type arguments.

Recommended help guidance:

1. provide at least two member types.


### `UNION_TYPE_UNSUPPORTED`

Severity: `error`

Trigger: a well-formed `Union[A, B, ...]` type appears outside a v0.1 `Res[T, E]` error-channel position.

Union types inside extern declaration signatures are ABI signature failures and MUST use `EXTERN_SIGNATURE_UNSUPPORTED`.

Message contract summary:

1. `union`.
2. `Res`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.


### `ORPHAN_RULE_VIOLATION`

Severity: `error`

Trigger: `impl Trait for Type` is declared where the canonical trait declaration
is non-local and the alias-expanded target either has no nominal target head or
has a nominal target head that is non-local to the current locality boundary.
Nested type arguments and structural wrappers do not make the target local.

Message contract summary:

1. `orphan`.
2. `local`.
3. `trait`.
4. `type`.


### `TRAIT_IMPL_DUPLICATE`

Severity: `error`

Trigger: two impl declarations have equivalent trait and target types plus alpha-equivalent generic parameters and order-insensitive normalized bound sets.

Message contract summary:

1. `impl`.
2. `duplicate`.
3. Trait name.
4. Target type name.


### `TRAIT_IMPL_OVERLAP`

Severity: `error`

Trigger: two distinct impl headers can both apply to at least one common monomorphized `(Trait, Type)` pair.

Message contract summary:

1. `impl`.
2. `overlap`.
3. Trait name.
4. Target type name.


### `SPECIALIZATION_FORBIDDEN`

Severity: `error`

Trigger: an impl is a strict specialization of another impl for the same trait in v0.1.

Message contract summary:

1. `specialization`.
2. `forbidden`.
3. Trait name.


### `NEGATIVE_IMPL_UNSUPPORTED`

Severity: `error`

Trigger: negative impl syntax (`impl !Trait for Type`) is used in v0.1.

Message contract summary:

1. `negative`.
2. `impl`.
3. `unsupported`.


### `IMPL_UNCONSTRAINED_TYPE_PARAM`

Severity: `error`

Trigger: an `impl[...]` generic parameter does not appear in either trait path or target type expression.

Message contract summary:

1. `impl`.
2. `type parameter`.
3. `unconstrained`.


### `IMPL_GENERIC_ERROR_UNION_UNSUPPORTED`

Severity: `error`

Trigger: after transparent-alias expansion and ordinary error-union
normalization, an impl-introduced generic parameter occurs strictly below a
remaining structural `Union[...]` node in the implemented trait reference or
target type.

Selection contract:

1. The restriction is checked before impl authority and coherence.
2. If more than one impl parameter violates the rule, select the earliest
   parameter in the impl's generic declaration list.
3. The primary span MUST identify the containing impl declaration.

Message contract summary:

1. `impl`.
2. `generic parameter`.
3. `error union`.
4. `unsupported`.

Data contract summary:

1. `parameter`: offending impl generic parameter name.


### `CONTRACT_NON_ATOMIC_AXIS`

Severity: `error`

Trigger: `contract{...}` uses non-atomic axis forms (`+`, `*`, numeric literal axis terms, grouped axis expressions, or axis-pack tokens such as `...`/`...name`).

Message contract summary:

1. `contract`.
2. `atomic`.
3. Offending token class (for example `+` or `*`) when available.


### `CONTRACT_OUTPUT_LABEL_DUPLICATE`

Severity: `error`

Trigger: the single `contract{...}` output axis list contains the same bound
label more than once.

Selection contract:

1. Output occurrences are checked left-to-right.
2. Each occurrence is checked for an input binding before duplicate detection.
3. The earliest bound occurrence that repeats an earlier bound output label
   triggers this code.

Message contract summary:

1. `contract`.
2. `output`.
3. `label`.
4. `duplicate`.
5. Offending output label when available.

Data contract summary (if `data` is emitted):

1. `operation`.
2. `label`.
3. `output_labels`.


### `CONTRACT_OUTPUT_LABEL_UNBOUND`

Severity: `error`

Trigger: `contract{...}` output axis list contains a label that does not appear in any left-hand-side operand axis list.

Selection contract:

1. Output occurrences are checked left-to-right.
2. The earliest unbound occurrence triggers this code before considering
   whether the same unbound label appears again.

Message contract summary:

1. `contract`.
2. `output`.
3. `label`.
4. Offending output label when available.

Data contract summary (if `data` is emitted):

1. `operation`.
2. `label`.
3. `input_labels`.
4. `output_labels`.


### `REARRANGE_OUTPUT_DTYPE_MISMATCH`

Severity: `error`

Trigger: after deterministic segment-provenance mapping, one `rearrange{...}`
output has contributors from input positions whose canonical element types are
not all equal.

Message contract summary:

1. `rearrange`.
2. `output`.
3. `dtype`.
4. `mismatch`.

Data contract summary:

1. `output_index`.
2. `left_type`.
3. `right_type`.

`left_type` is the canonical normalized element type of the first contributor
to the output. `right_type` is the canonical normalized element type of the
first later contributor that differs from it.


### `REDUCE_BY_ARGUMENTS_INVALID`

Severity: `error`

Trigger: receiver resolution commits a retained contextual `reduce_by`
candidate, but no argument interpretation is valid for the resolved receiver:

1. a canonical `ReduceSchema.reduce_by` argument sequence does not match the
   non-empty `ReduceByArgs` production, including an empty pack or mixed
   bare-reducer/phase forms; or
2. a non-`ReduceSchema` receiver uses phase-only tokens that do not form an
   ordinary `ArgList`.

The ordinary member-call candidate is not a fallback after canonical receiver
commitment, and the phase candidate is not available after a non-canonical
receiver commitment.

The parser MUST retain every token sequence that is valid ordinary member-call
syntax without consulting receiver identity. This diagnostic is emitted only
after receiver resolution commits the retained candidate. The primary span
MUST identify the complete parenthesized argument list.

Message contract summary:

1. `reduce_by`.
2. `arguments`.
3. `invalid`.


### `REDUCE_BY_PHASE_NON_REDUCED_AXIS`

Severity: `error`

Trigger: ordered-phase `reduce_by((axes_1, f_1), ...)` includes at least one axis not in the reduced-axis set of `reduce{...}`.

Message contract summary:

1. `reduce_by`.
2. `phase`.
3. `non-reduced`.
4. Offending axis label when available.

Data contract summary:

1. `axis`.
2. `phase_index`.


### `REDUCE_BY_PHASE_OVERLAP`

Severity: `error`

Trigger: ordered-phase `reduce_by((axes_1, f_1), ...)` assigns one reduced axis to two or more phases.

Validation scans phases in source order and the normalized axis set within each
phase in left-side schema-axis order. The primary span MUST identify the first
axis occurrence encountered after that axis was already assigned to an earlier
phase. `phase_index` is that later phase's 0-based index.

Message contract summary:

1. `reduce_by`.
2. `phase`.
3. `overlap`.
4. Duplicated axis label when available.

Data contract summary:

1. `axis`.
2. `phase_index`.


### `REDUCE_BY_PHASE_INCOMPLETE`

Severity: `error`

Trigger: ordered-phase `reduce_by((axes_1, f_1), ...)` does not cover the full reduced-axis set exactly once.

Message contract summary:

1. `reduce_by`.
2. `phase`.
3. `incomplete`.
4. `missing`.

Data contract summary:

1. `expected_axes`.
2. `actual_axes`.


### `AXIS_PACK_ANON_DUPLICATE`

Severity: `error`

Trigger: one transform literal (`view` / `rearrange` / `inflate` / `reduce` / `einop`) contains more than one anonymous axis-pack token `...`.

Message contract summary:

1. `axis pack`.
2. `anonymous`.
3. `duplicate`.


### `AXIS_PACK_UNBOUND`

Severity: `error`

Trigger: either:

1. an output-side named axis pack (`...name`) is not bound by any input-side
   named-pack occurrence in the same transform literal, or
2. an `einop` right-hand side contains an anonymous pack `...`, which cannot
   have an input binding under the one-anonymous-occurrence rule.

Message contract summary:

1. `axis pack`.
2. `unbound`.
3. Pack identifier when available.


### `AXIS_MAPPING_UNSUPPORTED`

Severity: `error`

Trigger: a tensor transform schema requires structural coordinate transport
outside the closed v0.1 mapping relation. This includes a distributive-looking
map whose repeated factor lineage does not match, or any map that would require
algebraic search beyond the written-tree-directed distributive and factoring
proof rules. Equal resolved extent or equal global numel does not make such a
mapping valid. It also includes any tagged-sum `+` node at any depth in a v0.1
`einop` left-hand-side or right-hand-side axis expression; `einop` branch routing
is outside the v0.1 mapping relation.

Message contract summary:

1. `axis`.
2. `mapping`.
3. `unsupported`.


### `EINOP_OUTPUT_LABEL_DUPLICATE`

Severity: `error`

Trigger: one `einop` right-hand-side output contains the same identifier or
named-pack token more than once. Each output is checked independently in
right-hand-side order, and occurrences within an output are checked in lexical
order. A repeated named pack triggers this code even when it could bind zero
axes. Integer-literal occurrences are anonymous and never trigger this code.

Message contract summary:

1. `einop`.
2. `output`.
3. `label`.
4. `duplicate`.
5. Offending identifier or pack name when available.

Data contract summary (if `data` is emitted):

1. `operation`.
2. `label`.
3. `output_index`.


### `IF_CONDITION_NOT_BOOL`

Severity: `error`

Trigger: `if` condition expression does not type-check to `Bool` or `Tri`.

Message contract summary:

1. `if`.
2. `Bool`.
3. `Tri`.
4. `condition`.


### `IF_TRI_UNKNOWN_BRANCH_REQUIRED`

Severity: `error`

Trigger: `if` condition has type `Tri` but `unknown { ... }` branch is missing.

Message contract summary:

1. `if`.
2. `Tri`.
3. `unknown`.
4. `required`.


### `IF_BOOL_UNKNOWN_BRANCH_FOR_BOOL`

Severity: `error`

Trigger: `if` condition has type `Bool` but `unknown { ... }` branch is present.

Message contract summary:

1. `if`.
2. `Bool`.
3. `unknown`.
4. `forbidden`.


### `IF_BRANCH_TYPE_MISMATCH`

Severity: `error`

Trigger: `if` branch result expressions cannot unify to a common type.

Message contract summary:

1. `if`.
2. `branch`.
3. `mismatch`.

Data contract summary:

1. `left_type`: the accumulated canonical common type immediately before the
   first failed source-ordered join.
2. `right_type`: the canonical type of the earliest later branch that cannot
   join it.


### `IF_MISSING_ELSE_VALUE`

Severity: `error`

Trigger: value context requires expression result but `if` is provided without `else`.

Message contract summary:

1. `if`.
2. `else`.
3. `value`.


### `MATCH_NON_EXHAUSTIVE`

Severity: `error`

Trigger: a `match` expression does not cover all possible scrutinee cases, or a
zero-arm match has a scrutinee whose bounded inhabitance fact is `Inhabited` or
`Unknown` rather than `Uninhabited`.

Message contract summary:

1. `match`.
2. `non-exhaustive`.


### `MATCH_UNREACHABLE_ARM`

Severity: `error`

Trigger: a `match` arm is unreachable because previous arms cover all remaining
possibilities or because its pattern can match only structurally impossible
variants.

Message contract summary:

1. `match`.
2. `unreachable`.
3. `arm`.


### `MATCH_GUARD_EFFECT`

Severity: `error`

Trigger: guard expression in `match` arm contains effectful call in v0.1.

Message contract summary:

1. `match`.
2. `guard`.
3. `effect`.


### `MATCH_GUARD_TYPE_MISMATCH`

Severity: `error`

Trigger: guard expression in `match` arm does not type-check to `Bool`.

Message contract summary:

1. `match`.
2. `guard`.
3. `Bool`.
4. `type`.

Data contract summary:

1. `expected_type` (`Bool`).
2. `actual_type`.


### `MATCH_BINDING_MISMATCH`

Severity: `error`

Trigger: alternatives within one or-pattern introduce different binding-name
sets or infer different types for the same binding name.

Message contract summary:

1. `match`.
2. `binding`.
3. `mismatch`.

Data contract summary:

1. `reason`: `missing_binding`, `extra_binding`, or `binding_type`, relative to
   the first alternative of the enclosing or-pattern.
2. `binding`: the selected binding name.
3. For `binding_type`, `left_type` is the canonical type inferred from the
   first alternative and `right_type` is the canonical type inferred from the
   first later alternative that differs.

For `missing_binding`, the selected name occurs in the first alternative but
not the failing later alternative. For `extra_binding`, it occurs only in that
later alternative. The primary span MUST identify the selected binding when it
exists in the failing alternative, or that complete alternative when the
binding is missing.


### `MATCH_ARM_TYPE_MISMATCH`

Severity: `error`

Trigger: `match` arm result expressions cannot unify to a common type.

Message contract summary:

1. `match`.
2. `arm`.
3. `mismatch`.

Data contract summary:

1. `left_type`: the accumulated canonical common type immediately before the
   first failed source-ordered join.
2. `right_type`: the canonical type of the earliest later arm that cannot join
   it.


### `PATTERN_TYPE_MISMATCH`

Severity: `error`

Trigger: a structurally resolved pattern form cannot match its contextual
expected type. This includes a tuple pattern against a non-tuple, `()` against
a non-`Unit` type, a struct or variant whose normalized nominal identity differs
from the expected type, or a valid non-interpolated literal whose kind/type is
incompatible with its expected type.

Contextual literal representability and domain validation precede this code and
retain their specific diagnostics, including `INT_OVERFLOW` and
`DOMAIN_INVALID_VALUE`.

Variant payload delimiter mismatch uses the more specific
`PATTERN_PAYLOAD_FORM_MISMATCH` after owner identity succeeds.

Message contract summary:

1. `pattern`.
2. `type`.
3. `mismatch`.

Data contract summary:

1. `reason`: `structural_form` or `typed_pattern`.
2. `pattern_kind`: `tuple`, `unit`, `struct`, `variant`, or `literal`.
3. `expected_type`: the canonical scrutinee type.
4. For `typed_pattern`, `actual_type` is the canonical nominal or literal type
   carried by the pattern. Structural forms MUST NOT invent an `actual_type`.

The primary span MUST identify the incompatible outer pattern form or head.


### `PATTERN_PAYLOAD_FORM_MISMATCH`

Severity: `error`

Trigger: a resolved variant pattern uses bare, parenthesized, or braced surface
that differs from the variant's declared unit, positional, or named-payload
form. A zero-arity positional variant still requires parentheses.

Message contract summary:

1. `pattern`.
2. `payload`.
3. `form`.
4. `mismatch`.

Data contract summary:

1. `owner`.
2. `variant`.
3. `expected_form`: `unit`, `positional`, or `named`.
4. `actual_form`: `unit`, `positional`, or `named`.

The primary span MUST identify the variant pattern head together with its
written payload delimiter when present; a bare form uses the head alone.


### `PATTERN_ARITY_MISMATCH`

Severity: `error`

Trigger: a tuple or positional-variant pattern cannot match the declared arity:

1. without `..`, its explicit item count differs from the target arity, or
2. with tail `..`, its explicit prefix contains more items than the target.

Message contract summary:

1. `pattern`.
2. `arity`.
3. `mismatch`.

Data contract summary:

1. `expected_count`: the target tuple or positional-payload arity.
2. `actual_count`: the pattern's explicit item count, excluding tail `..`.

The primary span MUST identify the tuple or variant payload pattern.


### `PATTERN_UNKNOWN_FIELD`

Severity: `error`

Trigger: a struct or named-payload variant pattern names a field absent from
the resolved target declaration.

Message contract summary:

1. `pattern`.
2. `field`.
3. `unknown`.

Data contract summary (if `data` is present):

1. `target`.
2. `field`.

The primary span MUST identify the first unknown field in source order.


### `PATTERN_DUPLICATE_FIELD`

Severity: `error`

Trigger: a struct or named-payload variant pattern writes the same field name
more than once.

Message contract summary:

1. `pattern`.
2. `field`.
3. `duplicate`.

Data contract summary (if `data` is present):

1. `target`.
2. `field`.

The primary span MUST identify the later occurrence of the first duplicate in
source order. This code precedes unknown- and missing-field validation for the
same container.


### `PATTERN_MISSING_FIELD`

Severity: `error`

Trigger: a struct or named-payload variant pattern without `..` omits one or
more fields declared by its resolved target.

Message contract summary:

1. `pattern`.
2. `field`.
3. `missing`.

Data contract summary (if `data` is present):

1. `target`.
2. `field`.

The first omitted field in declaration order is primary. Because no source
occurrence exists, the enclosing explicit pattern head MUST supply the primary
span; a pathless struct pattern uses its opening `{`.


### `PATTERN_DUPLICATE_BINDING`

Severity: `error`

Trigger: one complete alternative path through a structurally valid match
pattern introduces the same binding spelling more than once. Reusing one name
in separate valid alternatives of an or-pattern is not a duplicate; those
alternatives are compared by `MATCH_BINDING_MISMATCH` instead.

Message contract summary:

1. `pattern`.
2. `binding`.
3. `duplicate`.

Data contract summary (if `data` is present):

1. `name`.

The primary span MUST identify the later introduction of the first duplicate
on the first complete alternative path in source order.


### `PATTERN_OWNERSHIP_BINDING_OVERLAP`

Severity: `error`

Trigger: on one complete alternative of a conditional-root match pattern, two
different names bind the same ownership obligation. The primary span MUST
identify the second owning name on the first offending alternative in source
order. A related span SHOULD identify the first owning name.

Message contract summary:

1. `pattern`.
2. `ownership`.
3. `binding`.
4. `overlap`.


### `LOOP_CONDITION_NOT_BOOL`

Severity: `error`

Trigger: `while` condition expression does not type-check to `Bool`.

Message contract summary:

1. `while`.
2. `Bool`.
3. `condition`.


### `FOR_ITERABLE_NOT_SEQUENCE`

Severity: `error`

Trigger: `for-in` iterable operand does not provide sequence iteration semantics in v0.1.

Message contract summary:

1. `for`.
2. `in`.
3. `sequence`.


### `BREAK_OUTSIDE_LOOP`

Severity: `error`

Trigger: `break` appears with no enclosing loop in the same function or closure control-flow region.

Message contract summary:

1. `break`.
2. `loop`.
3. `outside`.


### `CONTINUE_OUTSIDE_LOOP`

Severity: `error`

Trigger: `continue` appears with no enclosing loop in the same function or closure control-flow region.

Message contract summary:

1. `continue`.
2. `loop`.
3. `outside`.


### `RETURN_OUTSIDE_FUNCTION`

Severity: `error`

Trigger: `return` appears with no enclosing function or method body, or appears inside a closure body where non-local return is not part of v0.1.

Message contract summary:

1. `return`.
2. `function`.
3. `outside`.


### `RETURN_VALUE_REQUIRED`

Severity: `error`

Trigger: bare `return` is used in function/method body whose return type is not `Unit`.

Message contract summary:

1. `return`.
2. `value`.
3. `required`.


### `RETURN_VALUE_FORBIDDEN`

Severity: `error`

Trigger: `return <expr>` is used in function/method body whose return type is `Unit`.

Message contract summary:

1. `return`.
2. `Unit`.
3. `forbidden`.


### `RETURN_TYPE_MISMATCH`

Severity: `error`

Trigger: function return-type contract is violated by value channel type:

1. an expression-body result is not assignable to the function/method return type,
2. expression type in `return <expr>` is not assignable to function/method return type, or
3. block-body computed result type (the common type of normal `yield` and
   fallthrough edges, or `Never` when no normal result edge exists) is not
   assignable to function/method return type.

Message contract summary:

1. `return`.
2. `type`.
3. `mismatch`.

Data contract summary:

1. `expected_type`: the canonical declared function or method result type.
2. `actual_type`: the canonical expression or computed block-result type that
   violates it.


### `LEX_INVALID_CHAR`

Severity: `error`

Trigger: lexer encounters a character not valid in any token class for current
context. This includes a `#` that is not the start of `#[` and is not followed
by an ASCII decimal digit. Outside comments and string literals it also
includes unsupported non-ASCII scalars without Unicode 17.0.0 `XID_Start` or
`XID_Continue` status, including Unicode whitespace and line-separator scalars
outside the source newline set.

The primary span MUST identify the first invalid scalar's exact UTF-8 byte
range.

Message contract summary:

1. `lex`.
2. `invalid`.
3. `char`.


### `LEX_INVALID_IDENTIFIER_CHAR`

Severity: `error`

Trigger: outside a comment or string literal, the lexer encounters a
non-ASCII Unicode scalar with Unicode 17.0.0 `XID_Start` or `XID_Continue`
status where v0.1's ASCII-only identifier grammar does not admit it.

The primary span MUST identify the first unsupported identifier scalar's exact
UTF-8 byte range.

Message contract summary:

1. `identifier`.
2. `ASCII`.
3. `invalid`.


### `LEX_INVALID_UTF8`

Severity: `error`

Trigger: source file byte stream is not valid UTF-8 before tokenization.

The primary span MUST identify the exact invalid physical byte sequence. It
MUST NOT include a derived text-coordinate projection because the source is
not valid Unicode scalar text.

Message contract summary:

1. `source`.
2. `UTF-8`.
3. `invalid`.


### `LEX_UNTERMINATED_STRING`

Severity: `error`

Trigger: string literal reaches end-of-line/file before closing quote.

Message contract summary:

1. `string`.
2. `unterminated`.


### `LEX_INVALID_ESCAPE`

Severity: `error`

Trigger: string literal contains an unsupported or malformed escape sequence.
This includes a `\u{...}` escape with no hexadecimal digits, more than six
hexadecimal digits, a non-hexadecimal digit, a surrogate value in
`U+D800..U+DFFF`, or a value above `U+10FFFF`.

Message contract summary:

1. `escape`.
2. `invalid`.


### `LEX_UNTERMINATED_BLOCK_COMMENT`

Severity: `error`

Trigger: block comment is opened but not closed before end-of-file.

Message contract summary:

1. `comment`.
2. `unterminated`.


### `LEX_NESTED_BLOCK_COMMENT`

Severity: `error`

Trigger: lexer encounters a `/*` opener inside an active block comment before that comment's matching `*/`.

Message contract summary:

1. `comment`.
2. `nested`.


### `LEX_INVALID_NUMBER`

Severity: `error`

Trigger: a started numeric literal, tuple-index token, or `AxisIndex` digit
component is malformed under v0.1 literal grammar.

Message contract summary:

1. `number`.
2. `invalid`.


### `FSTRING_UNRESOLVED_NAME`

Severity: `error`

Trigger: `f"..."` interpolation references a placeholder name that is not resolvable in lexical scope.

Message contract summary:

1. `f-string`.
2. `placeholder`.
3. `unresolved`.
4. Placeholder name when available.

Data contract summary (if `data` is present):

1. `name`.


### `FSTRING_INVALID_PLACEHOLDER`

Severity: `error`

Trigger: `f"..."` interpolation contains placeholder syntax outside v0.1 grammar (`{name}` or `{name:?}`), excluding escaped braces (`{{`, `}}`).

Message contract summary:

1. `f-string`.
2. `placeholder`.
3. `invalid`.


### `PLACEHOLDER_INVALID_CONTEXT`

Severity: `error`

Trigger: `_` hole expression appears outside a placeholder closure shorthand body or a call expression being lowered as hole-based partial application.

This code applies only to `_` in expression position as `HoleExpr`; wildcard patterns and discard assignment targets are not placeholder holes.

Message contract summary:

1. `placeholder`.
2. `hole`.
3. `invalid`.
4. `context`.


### `PARSE_CONTROL_HEAD_BRACE_REQUIRES_GROUPING`

Severity: `error`

Trigger: an `if`, `while`, `for-in`, or `match` control head attempts to use an
outer-depth brace as a struct literal body, named variant payload, or ordinary
trailing closure before the required control block. The brace-owning expression
MUST be enclosed in parentheses or another explicit delimiter.

This code takes precedence over generic parser diagnostics for that complete
restricted form. Diagnostic probing MUST NOT change the deterministic
control-body parse defined in `../language/syntax.md`.

Message contract summary:

1. `parse`.
2. `control head`.
3. `brace`.
4. `grouping`.


### `PARSE_UNEXPECTED_TOKEN`

Severity: `error`

Trigger: parser encounters a token not valid at the current grammar position,
or a context-sensitive call or suffix token sequence becomes invalid after
canonical receiver or callee identity selects its required grammar and that
surface does not assign a more specific diagnostic.

Message contract summary:

1. `parse`.
2. `unexpected`.
3. `token`.


### `PARSE_RESOURCE_LIMIT_EXCEEDED`

Severity: `error`

Trigger: the parser establishes that the next recursive grammar descent or
another parser-owned structural operation would exceed its deterministic
limit. The portable v0.1 recursive-grammar limit and implementation-defined
delimiter/event limits are specified in `../language/syntax.md`.

This code takes precedence over a grammar-invalid code caused only by the
parser's inability to continue beyond that limit. It does not replace an
independent root already established earlier in the source.

Message contract summary:

1. `parse`.
2. `resource`.
3. `limit`.
4. `exceeded`.

Data contract summary:

1. `resource` (`recursive_depth`, `delimiter_depth`, or `event_count`).
2. `limit` (the active non-negative parser limit as `i64`).
3. `observed_at_least` (the smallest established value greater than `limit` as
   `i64`).


### `PARSE_UNCLOSED_DELIMITER`

Severity: `error`

Trigger: parser reaches end-of-input or invalid boundary before closing delimiter (`)`, `]`, or `}`).

Message contract summary:

1. `parse`.
2. `unclosed`.
3. `delimiter`.


### `PARSE_EXPECTED_EXPRESSION`

Severity: `error`

Trigger: grammar requires an expression and none is present.

Message contract summary:

1. `parse`.
2. `expected`.
3. `expression`.


### `PARSE_EXPECTED_PATTERN`

Severity: `error`

Trigger: grammar requires a pattern (for example `match` arm head) and none is present.

Message contract summary:

1. `parse`.
2. `expected`.
3. `pattern`.


### `PARSE_INVALID_REST_POSITION`

Severity: `error`

Trigger: rest token `..` appears in a syntactically invalid pattern position in v0.1.

Message contract summary:

1. `parse`.
2. `rest`.
3. `position`.


### `PARSE_INVALID_RANGE_SYNTAX`

Severity: `error`

Trigger: range syntax `..`/`..=` is malformed (missing bound or invalid chaining).

Message contract summary:

1. `parse`.
2. `range`.
3. `syntax`.


### `RANGE_EXPR_UNSUPPORTED`

Severity: `error`

Trigger: range syntax `..`/`..=` appears outside a `for <name> in <range>` iterable expression.

Message contract summary:

1. `range`.
2. `for`.
3. `unsupported`.


### `MODULE_PATH_COMPONENT_INVALID`

Severity: `error`

Trigger: after package or file-set structural validation succeeds, a derived
canonical module-path component cannot be written scalar-for-scalar as one
ordinary source `Identifier` in its `ModulePath` position.

This diagnostic is emitted during resolve-phase build-input validation. The
applicable package or file-set structural checks and `MODULE_ROOT_RESERVED`
precede it. It precedes source decoding, lexing, parsing,
`MODULE_PATH_MISMATCH`, import resolution, and program-entry validation.

When multiple source units in the same package source tree or file set violate
the rule, the source selected by the canonical source-relative path order in
`../language/modules-resolution.md` determines the primary diagnostic. Within
that path, the leftmost invalid component determines diagnostic data. The
primary span MUST be the zero-width physical byte range `0..0` in that source
unit. It has a line-1, column-1 text projection when the source is valid UTF-8
and no text projection when the source is undecodable. An optional `module`
header is not the cause and MUST NOT receive primary blame.

Message contract summary:

1. `module`.
2. `path`.
3. `component`.
4. `invalid`.

Data contract summary:

1. `name` (exact derived component spelling).
2. `index` (exact zero-based component position).


### `MODULE_PATH_MISMATCH`

Severity: `error`

Trigger: optional `module <path>` declaration does not match canonical module path derived from file location.

Message contract summary:

1. `module`.
2. `path`.
3. `mismatch`.


### `MODULE_ROOT_RESERVED`

Severity: `error`

Trigger: after package or file-set structural validation succeeds, a
user-supplied `.eglur` source has a derived canonical module path whose first
segment is the reserved standard-library root `std`.

This diagnostic is emitted during resolve-phase build-input validation. The
applicable package or file-set structural checks precede it. It precedes source
decoding, lexing, parsing, `MODULE_PATH_MISMATCH`, import resolution, and
program-entry validation. Later path components do not affect this precedence.

When multiple source units in the same package source tree or file set violate
the rule, the source selected by the canonical source-relative path order in
`../language/modules-resolution.md` determines the primary diagnostic. The
primary span MUST be the zero-width physical byte range `0..0` in that source
unit. It has a line-1, column-1 text projection when the source is valid UTF-8
and no text projection when the source is undecodable. An optional `module`
header is not the cause and MUST NOT receive primary blame.

Message contract summary:

1. `module`.
2. `root`.
3. `std`.
4. `reserved`.

Data contract summary:

1. `name` (exact value `std`).


### `PACKAGE_MANIFEST_MISSING`

Severity: `error`

Trigger: package-mode compilation is selected, but the selected package root
lacks `Eglur.toml`. This includes a validated dependency source whose selected
root is not an Eglur package root.

Message contract summary:

1. `package`.
2. `manifest`.
3. `missing`.
4. `Eglur.toml`.

Data contract summary:

1. `package_role` (`root` or `dependency`).
2. `dependency` (required when `package_role = dependency`).
3. `source_kind` (`path` or `registry`; required for a dependency).
4. `path` (exact declared value; required for a path dependency).
5. `registry_package` (canonical `RegistryPackageId`; required for a registry
   dependency).


### `PACKAGE_SRC_ROOT_MISSING`

Severity: `error`

Trigger: package-mode compilation is selected, but the selected package root
lacks required `src/` directory.

Message contract summary:

1. `package`.
2. `src`.
3. `missing`.

Data contract summary:

1. `package_role` (`root` or `dependency`).
2. `dependency` (required when `package_role = dependency`).
3. `source_kind` (`path` or `registry`; required for a dependency).
4. `path` (exact declared value; required for a path dependency).
5. `registry_package` (canonical `RegistryPackageId`; required for a registry
   dependency).


### `PACKAGE_MANIFEST_PACKAGE_TABLE_MISSING`

Severity: `error`

Trigger: `Eglur.toml` exists, but does not define the required `[package]` table.

Message contract summary:

1. `package`.
2. `manifest`.
3. `package`.
4. `missing`.


### `PACKAGE_MANIFEST_NAME_MISSING`

Severity: `error`

Trigger: `Eglur.toml` defines `[package]`, but `package.name` is missing.

Message contract summary:

1. `package`.
2. `manifest`.
3. `name`.
4. `missing`.


### `PACKAGE_MANIFEST_NAME_INVALID`

Severity: `error`

Trigger: `Eglur.toml` defines `package.name`, but the value is not a valid
`PackageName` under `../language/package.md`.

Message contract summary:

1. `package`.
2. `manifest`.
3. `name`.
4. `invalid`.

Data contract summary:

1. `reason` (`non_string`, `empty`, or `invalid_character`).
2. `name` (exact decoded value; required for `empty` and `invalid_character`).


### `PACKAGE_MANIFEST_VERSION_MISSING`

Severity: `error`

Trigger: `Eglur.toml` defines `[package]`, but `package.version` is missing.

Message contract summary:

1. `package`.
2. `manifest`.
3. `version`.
4. `missing`.


### `PACKAGE_MANIFEST_VERSION_INVALID`

Severity: `error`

Trigger: `Eglur.toml` defines `package.version`, but the value is not a canonical
exact semantic version literal under `../language/package.md`, including when a
numeric component has a leading zero.

Message contract summary:

1. `package`.
2. `manifest`.
3. `version`.
4. `invalid`.

Data contract summary (if `data` is present):

1. `version`.


### `PACKAGE_REGISTRY_ALIAS_INVALID`

Severity: `error`

Trigger: a manifest `[registries]` table, alias key, authority value type, or
authority value violates the closed registry-alias contract in
`../language/package.md`.

Message contract summary:

1. `package`.
2. `registry`.
3. `alias`.
4. `invalid`.

Data contract summary:

1. `reason` (`table_shape`, `alias`, `authority_type`, or `authority`).
2. `alias` when a particular decoded alias entry is selected.
3. `authority` when a string authority value is available.


### `PACKAGE_DEP_KEY_INVALID`

Severity: `error`

Trigger: an immediate decoded dependency key under `[dependencies]` in
`Eglur.toml` or `Eglur.lock` is not a source-addressable dependency import-root
key under `../language/package.md`.

This diagnostic is emitted during resolve-phase package validation. Manifest
key validation precedes dependency-entry selector validation, dependency-graph
validation, and all lockfile checks. Lockfile key validation follows lockfile
presence validation and precedes every `PACKAGE_LOCK_OUT_OF_SYNC` mismatch
category.

When an artifact contains multiple invalid keys, selection follows the
canonical lexical decoded-key order defined by `../language/package.md`. The
primary span MUST identify the complete TOML key spelling that decoded to the
selected invalid key, including quoting delimiters when present.

Message contract summary:

1. `package`.
2. `dependency`.
3. `key`.
4. `invalid`.

Data contract summary:

1. `dependency` (the decoded invalid key).


### `PACKAGE_VERSION_RANGE_UNSUPPORTED`

Severity: `error`

Trigger: a registry dependency uses a syntactically recognized version range
instead of an exact version literal.

Message contract summary:

1. `version`.
2. `range`.
3. `unsupported`.

Data contract summary:

1. `dependency`.
2. `version_spec`.


### `PACKAGE_DEP_SOURCE_CONFLICT`

Severity: `error`

Trigger: a dependency entry is not a table, contains unsupported fields that
prevent a closed source form, indicates both path and registry sources, or does
not indicate any source.

Message contract summary:

1. `package`.
2. `dependency`.
3. `source`.
4. `conflict`.

Data contract summary:

1. `dependency`.
2. `reason` (`entry_shape`, `unsupported_field`, `mixed_source`, or
   `missing_source`).


### `PACKAGE_REGISTRY_SELECTOR_INVALID`

Severity: `error`

Trigger: a dependency's registry/package fields cannot normalize to exactly one
canonical `RegistryPackageId` under `../language/package.md`.

Message contract summary:

1. `package`.
2. `registry`.
3. `selector`.
4. `invalid`.

Data contract summary:

1. `dependency`.
2. `reason` (`missing_package`, `non_string`, `unknown_alias`,
   `qualified_with_alias`, `unqualified_without_alias`, or `invalid_package`).
3. `registry` and `package` when those decoded string fields are available.


### `PACKAGE_DEP_VERSION_INVALID`

Severity: `error`

Trigger: a registry dependency's required exact `version` is missing,
non-string, or malformed, including a leading-zero numeric component, and is
not a syntactically recognized range.

Message contract summary:

1. `package`.
2. `dependency`.
3. `version`.
4. `invalid`.

Data contract summary:

1. `dependency`.
2. `reason` (`missing`, `non_string`, or `malformed`).
3. `version` when a string value is available.


### `PACKAGE_DEP_PATH_INVALID`

Severity: `error`

Trigger: a manifest path dependency selector is empty, uses forbidden absolute
syntax (including recognized drive or UNC forms), or contains a backslash under
`../language/package.md`.

Message contract summary:

1. `package`.
2. `dependency`.
3. `path`.
4. `invalid`.

Data contract summary:

1. `dependency`.
2. `path` (exact declared value).
3. `reason` (`empty`, `absolute`, or `backslash`).


### `PACKAGE_DEP_PATH_UNRESOLVED`

Severity: `error`

Trigger: a valid canonical path selector cannot resolve to a directory because
the path is absent, a required target is not a directory, a symbolic link is
broken, or symbolic-link traversal cycles.

Message contract summary:

1. `package`.
2. `dependency`.
3. `path`.
4. `unresolved`.

Data contract summary:

1. `dependency`.
2. `path` (exact declared value).
3. `reason` (`not_found`, `not_directory`, `broken_symlink`, or
   `symlink_cycle`).


### `PACKAGE_LOCK_MISSING`

Severity: `error`

Trigger: package-mode compilation is selected and required top-level
`Eglur.lock` is absent.

Message contract summary:

1. `package`.
2. `lock`.
3. `missing`.
4. `Eglur.lock`.


### `PACKAGE_LOCK_INVALID`

Severity: `error`

Trigger: `Eglur.lock` is malformed or internally noncanonical without needing
manifest, filesystem, or registry-artifact evidence. This includes invalid TOML
or closed schema, source variants, node IDs/order, edges, reachability, source
compatibility, path text, tree-integrity spelling, and contradictory registry
coordinates. Invalid decoded dependency keys retain
`PACKAGE_DEP_KEY_INVALID`.

Message contract summary:

1. `package`.
2. `lock`.
3. `invalid`.


### `PACKAGE_LOCK_OUT_OF_SYNC`

Severity: `error`

Trigger: a structurally valid complete `Eglur.lock` disagrees with a reached
package manifest or with resolved path-package topology under
`../language/package.md`.

Message contract summary:

1. `package`.
2. `lock`.
3. `out of sync`.

Data contract summary:

1. `reason` (`package_metadata`, `dependency_key_set`, or
   `dependency_selector`).
2. `node` (the selected `LockedNodeId`, including `$root`).
3. `dependency` for a selected dependency-selector mismatch.


### `PACKAGE_REGISTRY_RELEASE_MISSING`

Severity: `error`

Trigger: the provider reports that the exact canonical `RegistryPackageId` and
version selected by the validated lock do not exist. No partial acquisition or
candidate tree is available.

Message contract summary:

1. `package`.
2. `registry`.
3. `release`.
4. `missing`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `version`.


### `PACKAGE_REGISTRY_TRANSPORT_FAILED`

Severity: `error`

Trigger: registry fetching or provider-protocol decoding fails before one
complete logical entry source is sealed. Partial entries are discarded and MUST
NOT affect this diagnostic.

Message contract summary:

1. `package`.
2. `registry`.
3. `transport`.
4. `failed`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `version`.


### `PACKAGE_REGISTRY_LIMIT_EXCEEDED`

Severity: `error`

Trigger: the compiler-owned private acquisition sink observes the first logical
resource value beyond the mandatory v0.1 registry limit. Partial entries are
discarded before tree validation.

Message contract summary:

1. `package`.
2. `registry`.
3. `limit`.
4. `exceeded`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `version`.
3. `resource` (`entry_count`, `path_bytes`, `path_depth`,
   `link_target_bytes`, `total_path_and_target_bytes`, `regular_file_bytes`, or
   `total_regular_file_bytes`).
4. `limit` (the normative v0.1 maximum as `i64`).
5. `observed_at_least` (exactly `limit + 1` as `i64`).


### `PACKAGE_REGISTRY_INTEGRITY_MISMATCH`

Severity: `error`

Trigger: the complete canonical registry package tree does not match the
`RegistryTreeIntegrity` selected by the validated lock. Transport-container
bytes and metadata are not compared by this diagnostic.

Message contract summary:

1. `package`.
2. `registry`.
3. `integrity`.
4. `mismatch`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `version`.
3. `expected` (canonical `tree-sha256:` digest from the lock).
4. `actual` (canonical `tree-sha256:` digest of the complete validated canonical
   package tree).


### `PACKAGE_REGISTRY_TREE_INVALID`

Severity: `error`

Trigger: a complete acquired registry release cannot form the canonical package
tree defined by `../language/package.md`. This includes invalid path encoding,
an otherwise invalid path, unsupported entry kind, exact duplicate regular-file
path, Unicode simple-case-fold path collision, or file/ancestor conflict. This
diagnostic follows acquisition-limit checks and precedes tree hashing and
integrity comparison.

Message contract summary:

1. `package`.
2. `registry`.
3. `tree`.
4. `invalid`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `version`.
3. `reason` (`invalid_path_encoding`, `invalid_path`,
   `unsupported_entry_kind`, `duplicate_path`, `case_collision`, or
   `file_ancestor`).
4. `path_hex` (the selected raw path as lowercase hexadecimal) when `reason` is
   `invalid_path_encoding`.
5. `path` (the selected offending decoded path) for every other reason.
6. `entry_kind` when `reason` is `unsupported_entry_kind`.
7. `existing_path` when `reason` is `case_collision` or `file_ancestor`.


### `PACKAGE_DEP_DUPLICATE`

Severity: `error`

Trigger: two distinct direct dependency keys in one declaring package resolve
to the same `PackageInstanceId` under `../language/package.md`.

Message contract summary:

1. `package`.
2. `dependency`.
3. `duplicate`.

Data contract summary:

1. `package` (declaring `PackageName`).
2. `dependency` (later canonical dependency key).
3. `existing_dependency` (first key bound to the instance).


### `PACKAGE_DEP_CYCLE`

Severity: `error`

Trigger: the locked and resolved package dependency graph contains a cycle.

Message contract summary:

1. `package`.
2. `dependency`.
3. `cycle`.


### `PACKAGE_VERSION_CONFLICT`

Severity: `error`

Trigger: one graph selects more than one exact version for one canonical
`RegistryPackageId`.

Message contract summary:

1. `package`.
2. `version`.
3. `conflict`.

Data contract summary:

1. `package` (canonical `RegistryPackageId`).
2. `existing_dependency`.
3. `dependency`.
4. `existing_version`.
5. `version`.


### `PACKAGE_NAME_CONFLICT`

Severity: `error`

Trigger: two distinct `PackageInstanceId` values in one resolved graph declare
the same `PackageName`, including a conflict with the root package.

Message contract summary:

1. `package`.
2. `name`.
3. `conflict`.

Data contract summary:

1. `package`.
2. `existing_dependency`.
3. `dependency`.
4. `existing_version`.
5. `version`.
6. `existing_source_kind`.
7. `source_kind`.
8. `reason` (`source` or `version_and_source`).

Host-local resolved roots and graph-local package instance IDs MUST NOT be
emitted.


### `IMPORT_NOT_FOUND`

Severity: `error`

Trigger: `import` path or imported symbol cannot be resolved. Nominal-sum
variants are not standalone import symbols; importing a source enum or catalog
sum variant directly uses this code instead of introducing a free constructor
name.

Message contract summary:

1. `import`.
2. `not found`.


### `IMPORT_ROOT_CONFLICT`

Severity: `error`

Trigger: in package mode, import first segment is ambiguous between a direct dependency key and a same-root local module path.

Message contract summary:

1. `import`.
2. `root`.
3. `conflict`.
4. First-segment name.


### `IMPORT_SUPER_AT_ROOT`

Severity: `error`

Trigger: relative import uses `super::` in a module that has no parent.

Message contract summary:

1. `super`.
2. `root`.
3. `import`.


### `IMPORT_BANG_ALIAS_MUST_BANG`

Severity: `error`

Trigger: bang import target (`name!`) is aliased to a non-bang identifier.

Message contract summary:

1. `bang`.
2. `alias`.
3. `!`.


### `NAME_AMBIGUOUS`

Severity: `error`

Trigger:

1. unqualified name resolves to multiple candidates in the same resolution tier, or
2. duplicate member declarations appear in one shared struct-member namespace (for example across `field` / `field mut` / `prop` / `prop mut` declarations), or
3. a bare pattern identifier could be read as either a new binding or a variant
   of the scrutinee's nominal sum type; v0.1 requires the owner-qualified
   variant spelling instead, or
4. one trait declares the same associated type name more than once.

Message contract summary:

1. `ambiguous`.
2. `name`.

Data contract summary (if `data` is present):

1. `name`.
2. `candidates`.


### `NAME_NOT_FOUND`

Severity: `error`

Trigger: after context-specific classification, either an ordinary unqualified
identifier lookup has no candidate in any applicable local, module,
explicit-import, or prelude resolution tier, or a struct/variant constructor-head
path has no candidate in its applicable lookup namespace. A form that defines
a more specific diagnostic keeps that code, including
`ASSIGN_TARGET_NOT_ASSIGNABLE` for an unresolved assignment target and
`FSTRING_UNRESOLVED_NAME` for an unresolved interpolated-string placeholder.
This code does not apply to a failed `import` path or imported symbol, which uses
`IMPORT_NOT_FOUND`.
The type-only prelude binding `Unit` is not a value candidate, so bare `Unit` in
expression position uses this code unless an ordinary value binding with that
spelling is in scope.
A source enum or catalog sum owner is likewise type-only in expression position.
Dot member syntax such as `Flag.On` or `Tri.Unknown` therefore uses this code
when no ordinary value binding named `Flag` or `Tri` shadows the type.
A struct literal head uses type-namespace lookup. If that lookup selects a
transparent alias, the name was found and `CONSTRUCTOR_TARGET_KIND_MISMATCH`
applies instead. An owner-qualified variant head still uses this code when its
constructor lookup finds no candidate, including an alias-qualified path that
does not resolve in that namespace.

Message contract summary:

1. `name`.
2. `not found`.

Data contract summary (if `data` is present):

1. `name`.


### `VISIBILITY_VIOLATION`

Severity: `error`

Trigger:

1. code accesses a non-public top-level item from outside its defining module,
2. code outside a struct's defining module reads or writes a non-public stored field, reads a non-public property, directly calls one of that property's required accessors, or calls/references a non-public inherent method family,
3. code outside a struct's defining module uses a struct literal when the struct or any required stored field is not externally accessible, or
4. an externally reachable type contract contains a nominal type or trait
   contract that is not externally nameable after transparent alias expansion.

The fourth form covers public free-function signatures, public declaration,
method, and conformance bounds, externally accessible fields, properties, and inherent methods,
public trait method contracts, public enum payloads, `pub const` types, `pub
type` targets, externally reachable conformance headers, and associated-type
bindings. It includes a private nominal or trait declaration and, in package
mode, a declaration owned
by another package without an exact-target `pub import` in the contract-owning
module. It is rejected at the owning declaration rather than at a later import,
call, projection, consumer manifest edit, or inferred-value use.
Language-intrinsic and externally accessible catalog type heads count as
nameable. A private source alias whose expansion is nameable does not trigger
this code, while a public alias does not repair a private target or a missing
foreign-package re-export bridge. For this diagnostic, an unbridged dependency
declaration is a private implementation detail of the package that owns the
public contract.

Primary-span requirement:

1. Direct cross-module item or member access selects the inaccessible reference,
   import, member, or method occurrence at the access site.
2. Inaccessible external struct construction selects the constructor head.
3. An externally reachable type contract that reaches an unnameable declaration
   selects the outermost source type occurrence in that contract whose expansion
   first reaches that declaration. For an associated binding, this is the
   selected occurrence in the binding right-hand side.
4. When one declaration contains multiple failing contract positions, selection
   uses source order. Within one position, selection uses the left-to-right
   structural walk defined by `modules-resolution.md`.
5. A private declaration or the private import that lacks the required public
   re-export MAY be reported as a related span.

Message contract summary:

1. `visibility`.
2. `private`.


### `TYPE_ALIAS_CYCLE`

Severity: `error`

Trigger: type-alias expansion graph contains a cycle.

Message contract summary:

1. `type`.
2. `alias`.
3. `cycle`.


### `FN_RETURN_TYPE_REQUIRED`

Severity: `error`

Trigger: named `fn`/`fn!` or `extern "..." fn` declaration omits explicit return type in v0.1.

Message contract summary:

1. `return`.
2. `type`.
3. `required`.


### `BINDING_TYPE_UNDETERMINED`

Severity: `error`

Trigger:

1. Local binding declaration omits its initializer expression.
2. Local binding initializer is present, but binding type remains underdetermined after local inference constraints (for example `let x = None` or `let x = [:]`), unless an expression-family rule assigns a more specific primary diagnostic. Generic nominal-constructor owner inference uses `TYPE_ARG_INFERENCE_FAILED`.
3. A stored `ReduceBySchema` initializer has an input-dependent inline callback
   whose parameter or other callback variables remain unresolved at the
   construction site. A later invocation is not initializer context.

Message contract summary:

1. `binding`.
2. `type`.
3. `initializer`.

Recommended help guidance:

1. add the missing initializer or explicit local/callback type context, or
2. invoke `reduce{...}.reduce_by(...)` in the same expression as its input
   instead of storing an underdetermined schema.


### `LOCAL_BINDING_REDECLARATION`

Severity: `error`

Trigger:

1. A local `let`/`let mut` binding introduces a name already introduced in the
   same lexical block scope.
2. A function, method, trait method, extern, or closure parameter list
   introduces the same value-parameter name more than once.

Nested lexical scopes may shadow outer bindings and do not trigger this code.
A duplicate name within one local `let` pattern uses the more specific
`LET_PATTERN_DUPLICATE_BINDING`; valid or-pattern alternatives that establish
one common arm binding set are not redeclarations.

Same-scope collision validation MUST precede initializer name resolution, type
inference, effect checking, and ownership analysis. Syntax, reserved-name,
signature-shape, unsupported-pattern, and intra-pattern duplicate diagnostics
retain their more specific precedence.

The primary span MUST identify the second introduction of the first colliding
name in source order.

Message contract summary:

1. `local`.
2. `binding`.
3. `redeclared`.

Data contract summary (if `data` is present):

1. `name`.


### `PARAM_PATTERN_UNSUPPORTED`

Severity: `error`

Trigger: function, method, trait, or extern parameter declaration uses a non-identifier pattern in v0.1.

Message contract summary:

1. `parameter`.
2. `pattern`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `position`.
2. `pattern_kind`.


### `LET_PATTERN_UNSUPPORTED`

Severity: `error`

Trigger: local `let`/`let mut` binding uses a pattern outside the v0.1 accepted
local binding subset of identifier, wildcard `_`, Unit pattern `()`, and tuple
patterns recursively composed of accepted local binding patterns.

Message contract summary:

1. `let`.
2. `pattern`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `pattern_kind`.


### `LET_PATTERN_TYPE_MISMATCH`

Severity: `error`

Trigger: Unit local binding pattern `()` is applied to a non-`Unit` initializer
type, a tuple local binding pattern is applied to a non-tuple initializer type,
or nested tuple arity does not match the initializer tuple type.

Message contract summary:

1. `let`.
2. `pattern`.
3. `type`.
4. `mismatch`.

Data contract summary:

1. `reason`: `tuple_required`, `tuple_arity`, or `unit_required`.
2. `pattern_kind`: `tuple` or `unit`.
3. `actual_type`: the canonical initializer type.
4. For `unit_required`, `expected_type` is `Unit`.
5. For `tuple_arity`, `expected_count` is the pattern arity and `actual_count`
   is the initializer tuple arity at the first mismatching nesting level.


### `LET_PATTERN_DUPLICATE_BINDING`

Severity: `error`

Trigger: one local binding pattern introduces the same identifier more than once.

Message contract summary:

1. `let`.
2. `pattern`.
3. `duplicate`.

Data contract summary (if `data` is present):

1. `name`.


### `IMMUTABLE_BINDING_ASSIGN`

Severity: `error`

Trigger: assignment targets an immutable local `let` binding or immutable `field` member.

Message contract summary:

1. `immutable`.
2. `assignment`.
3. Target name or kind when available.


### `ASSIGNMENT_TYPE_MISMATCH`

Severity: `error`

Trigger: a fully typed initializer of an annotated `let`, `let mut`, or `const`
declaration is not assignable to the annotation, or a fully typed assignment
RHS is not assignable to a valid writable storage target. Local, stored-field,
and explicit-dereference assignments share this code.

Declaration/name/pattern-form validation and assignment target/writability
validation retain precedence. The initializer or RHS is typed with its expected
type before compatibility is checked; expression-internal errors retain their
own codes. Annotation compatibility precedes destructuring shape checks.
Compatibility precedes downstream effect/ownership validation. Call and
constructor arguments and return channels retain their dedicated codes.

The primary span MUST identify the complete incompatible initializer or RHS
expression, not the declaration name, assignment target, or annotation. For a
tuple-valued initializer this is the whole initializer, not one element.

Message contract summary:

1. `assignment`.
2. `type`.
3. `mismatch`.

Data contract summary:

1. `expected_type`: the canonical annotation or target storage type.
2. `actual_type`: the canonical initializer or RHS type.


### `ASSIGN_TARGET_NOT_ASSIGNABLE`

Severity: `error`

Trigger: assignment target expression is not an assignable v0.1 storage target.

Representative invalid targets include unresolved bare identifier assignments, call results, `?` expressions, and other value-producing `PostfixExpr` forms.

Message contract summary:

1. `assignment`.
2. `target`.
3. `assignable`.

Data contract summary (if `data` is present):

1. `target_kind`.


### `VAR_DECL_FORBIDDEN`

Severity: `error`

Trigger: `var` keyword is used as a declaration introducer in v0.1 (local binding or struct/trait member declaration).

Phase policy:

1. Detection MAY occur during parsing or later static phases.
2. Regardless of detection phase, emitted code MUST be `VAR_DECL_FORBIDDEN` (not a generic parse/type fallback code).

Message contract summary:

1. `var`.
2. `forbidden`.


### `ENUM_VARIANT_VISIBILITY_FORBIDDEN`

Severity: `error`

Trigger: enum declaration attempts per-variant visibility modifier in v0.1 baseline (for example `enum E { pub A }`).

Message contract summary:

1. `enum`.
2. `variant`.
3. `visibility`.
4. `forbidden`.


### `ATTRIBUTE_UNSUPPORTED`

Severity: `error`

Trigger: attribute name is parse-valid under attribute-lite grammar but has no v0.1 semantic support.

Message contract summary:

1. `attribute`.
2. `unsupported`.
3. Attribute name token.

Data contract summary (if `data` is present):

1. `attribute`.


### `ATTRIBUTE_TARGET_UNSUPPORTED`

Severity: `error`

Trigger: attribute name is supported in v0.1, but appears on a semantic target outside that attribute's supported target set, and no more specific target diagnostic applies.

Message contract summary:

1. `attribute`.
2. `target`.
3. `unsupported`.
4. Attribute name token.

Data contract summary (if `data` is present):

1. `attribute`.
2. `target`.


### `DERIVE_TARGET_UNSUPPORTED`

Severity: `error`

Trigger: `#[derive(...)]` appears on a v0.1 semantic target other than `struct` or `enum`.

Message contract summary:

1. `derive`.
2. `target`.
3. `struct`.
4. `enum`.

Data contract summary (if `data` is present):

1. `target`.


### `DERIVE_DUPLICATE_ENTRY`

Severity: `error`

Trigger: one `derive(...)` list repeats the same normalized trait path more than once.

Message contract summary:

1. `derive`.
2. `duplicate`.
3. `trait`.

Data contract summary (if `data` is present):

1. `trait`.


### `DERIVE_TRAIT_RESOLUTION_FAILED`

Severity: `error`

Trigger: derive entry trait path is unresolved, not visible, or resolves to a non-trait symbol.

Message contract summary:

1. `derive`.
2. `trait`.
3. `resolve`.
4. `failed`.

Data contract summary (if `data` is present):

1. `trait`.


### `DERIVE_TRAIT_UNSUPPORTED`

Severity: `error`

Trigger: derive entry trait path resolves, but no active derive support/provider exists for that trait in the current profile.

Message contract summary:

1. `derive`.
2. `trait`.
3. `unsupported`.

Data contract summary (if `data` is present):

1. `trait`.


### `DERIVE_ELIGIBILITY_FAILED`

Severity: `error`

Trigger: resolved derive support/provider rejects the annotated `struct`/`enum` because required payload, structural, or generic-bound constraints are not satisfied.

Built-in derive `reason` taxonomy used by v0.1 conformance:

1. `missing_required_bound`
2. `missing_required_eq`
3. `enum_default_missing`
4. `enum_default_multiple`
5. `enum_default_non_unit`
6. `recursive_type_argument_growth`

Message contract summary:

1. `derive`.
2. `eligibility`.
3. `failed`.

Data contract summary (if `data` is present):

1. `trait`.
2. `reason`.


### `TEST_ATTRIBUTE_TARGET_UNSUPPORTED`

Severity: `error`

Trigger: `#[test]` is applied to a semantic target other than a top-level function declaration.

Message contract summary:

1. `test`.
2. `attribute`.
3. `target`.
4. `top-level`.
5. `function`.

Data contract summary (if `data` is present):

1. `target`.


### `TEST_FUNCTION_INVALID_SIGNATURE`

Severity: `error`

Trigger: `#[test]` function violates v0.1 signature rules (non-zero parameters,
generic parameters, or return type outside `Unit` / `Res[Unit,E]`). `Never` is
not a test result protocol.

Message contract summary:

1. `test`.
2. `signature`.
3. `Unit`.
4. `Res`.

Data contract summary (if `data` is present):

1. `function`.


### `TEST_IGNORE_REQUIRES_TEST`

Severity: `error`

Trigger: `#[ignore]` appears on a declaration that does not also carry `#[test]` on the same top-level function.

If the same declaration also carries `#[test]` but is not a top-level function, `TEST_ATTRIBUTE_TARGET_UNSUPPORTED` takes precedence.

Message contract summary:

1. `ignore`.
2. `requires`.
3. `test`.

Data contract summary (if `data` is present):

1. `function`.


### `STRUCT_FIELD_DEFAULT_FORBIDDEN`

Severity: `error`

Trigger: struct field declaration includes a default initializer (`= expr`) in v0.1.

Message contract summary:

1. `struct`.
2. `field`.
3. `default`.
4. `forbidden`.

Data contract summary (if `data` is present):

1. `struct`.
2. `field`.


### `CONST_INIT_NOT_CONST`

Severity: `error`

Trigger: `const` initializer is not const-evaluable under `../language/validation-matrix.md`.

Precedence note:

1. This code is the required primary diagnostic for non-const-evaluable initializers, even when nested expressions contain cast operations that could fail under const evaluation.
2. Cast-specific primary codes (`CAST_INVALID_DTYPE_PAIR`, `CAST_INVALID_VALUE`) apply only after const-evaluable classification succeeds.

Message contract summary:

1. `const`.
2. `initializer`.
3. `const-evaluable`.


### `CONST_FORWARD_REFERENCE`

Severity: `error`

Trigger: same-module `const` initializer references a constant declared later in source order, and no dependency cycle is present.

Message contract summary:

1. `const`.
2. `forward`.
3. `reference`.


### `CONST_CYCLE`

Severity: `error`

Trigger: constant dependency expansion contains a cycle (including cross-module cycles).

Message contract summary:

1. `const`.
2. `cycle`.
3. `dependency`.


### `OWNERSHIP_PATH_UNBALANCED`

Severity: `error`

Trigger: for enabled ownership surface, a typed control-flow edge exits a
lexical obligation region while a root belonging to that region is not
definitely consumed after accounting for transfers through the edge.

This applies to bang and non-bang bodies, direct roots `Own[T]`, conditional roots `Opt[Own[T]]` / `Res[Own[T], E]`, and local roots introduced by fresh or moved ownership values.

Roots belonging to enclosing regions that remain active after an inner block,
`result`, or `checked` exit do not trigger this code. The primary span MUST
identify the earliest explicit exiting construct in source order (`yield`,
`?`, checked failing operation, `break`, `continue`, or `return`), or the
closing delimiter for the earliest failing normal-completion edge.

Message contract summary:

1. `ownership`.
2. `path`.
3. `exactly once`.


### `OWNERSHIP_TYPE_POSITION_FORBIDDEN`

Severity: `error`

Trigger: alias-expanded type structure contains `Own[...]` outside a complete direct root, an allowed borrow view, or the exact conditional root shapes `Opt[Own[T]]` and `Res[Own[T], E]` with ownership-free, reference-free payload arguments.

This includes ownership-bearing user fields/payloads, tuples, ordinary containers, callable types or capture environments, nested carriers, `Res` error payloads, ordinary generic instantiations, and associated type bindings.

Message contract summary:

1. `Own`.
2. `type position`.
3. `forbidden`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.


### `REFERENCE_TYPE_POSITION_FORBIDDEN`

Severity: `error`

Trigger: an ordinary generic argument or associated type binding contains `&` or `&mut` after transparent alias expansion and normalization of every currently resolvable associated projection. The check includes nested tuple elements, generic arguments, and `Fn` parameter and result types. An outer borrow such as `&T` or `&T::Item` is not a reference-bearing abstraction argument and remains governed by ordinary reference rules.

Name, alias, arity, and projection validity are checked first. A cycle encountered while normalizing an associated binding uses `ASSOCIATED_TYPE_CYCLE`. For every finite normalized type, `OWNERSHIP_TYPE_POSITION_FORBIDDEN` is primary when the same type also violates the ownership-position rules. This diagnostic otherwise precedes bound satisfaction and any `NON_BANG_MUT_PARAM_FORBIDDEN` or `REF_ESCAPE_FORBIDDEN` failure that would arise only after the forbidden substitution.

Message contract summary:

1. `reference`.
2. `type position`.
3. `forbidden`.

Data contract summary (if `data` is present):

1. `position`.
2. `type`.


### `OWNERSHIP_TRANSFER_REQUIRES_MOVE`

Severity: `error`

Trigger: a named ownership root is used in a consuming position without the explicit `move <identifier>` form, including a by-value root argument, root result, carrier construction, `?`, or consuming `match` scrutinee.

Message contract summary:

1. `ownership`.
2. `transfer`.
3. `move`.

Data contract summary (if `data` is present):

1. `value`.
2. `position`.


### `OWNERSHIP_DISCARD_FORBIDDEN`

Severity: `error`

Trigger: an ownership-bearing value would be discarded or overwritten outside an allowed explicit discharge, including `_ = expr`, an unused expression statement, assignment over an existing root, or `_ = move x`. In a consuming match this includes an arm that can select a present owned payload without binding it or its carrier.

Message contract summary:

1. `ownership`.
2. `discard`.
3. `forbidden`.

Data contract summary (if `data` is present):

1. `position`.


### `MOVE_REQUIRES_OWN`

Severity: `error`

Trigger: ownership `move` operator is applied to an identifier whose type is not a direct or conditional ownership-root type.

Message contract summary:

1. `move`.
2. `Own`.
3. `operand`.

Data contract summary (if `data` is present):

1. `value`.

Recommended related-span guidance:

1. when a distinct prior consume location exists, a `related` note/span SHOULD describe the first consume location.


### `OWN_REQUIRES_NON_OWN`

Severity: `error`

Trigger: ownership `own` operator is applied to an operand that already has an ownership-root type.

Message contract summary:

1. `own`.
2. `Own`.
3. `non-Own`.

Data contract summary (if `data` is present):

1. `value`.


### `OWN_REQUIRES_LOCAL_BINDING`

Severity: `error`

Trigger: `own <identifier>` resolves its operand to a const or module/global binding rather than an available ownership-free local binding or function/method parameter.

Message contract summary:

1. `own`.
2. `local binding`.
3. `required`.

Data contract summary (if `data` is present):

1. `value`.


### `DROP_OWN_REQUIRES_MOVE`

Severity: `error`

Trigger: in an ownership-enabled profile, parse-valid `drop_own <identifier>` is used without the explicit `move` operand form (`drop_own move <identifier>`).

Message contract summary:

1. `drop_own`.
2. `move`.
3. `required`.


### `RESOURCE_DROP_FORBIDDEN`

Severity: `error`

Trigger: `drop_own move <identifier>` is applied to a direct or conditional ownership root whose `Own[...]` payload is a designated external resource type.

Resource roots require an effect-visible consuming `close!` operation or a standard-library scoped resource API; generic ownership discard cannot release them.

Message contract summary:

1. `resource`.
2. `drop_own`.
3. `close!`.

Data contract summary (if `data` is present):

1. `value`.
2. `resource_type`.


### `OWNERSHIP_DOUBLE_MOVE`

Severity: `error`

Trigger: a consuming ownership operation (`move`, `drop_own move`, consuming resource terminal, ownership-aware `?`, or `match move`) is applied to the same ownership root after it was already consumed on that typed path.

Message contract summary:

1. `ownership`.
2. `move`.
3. `already consumed`.

Data contract summary (if `data` is present):

1. `value`.

Recommended related-span guidance:

1. when a distinct prior consume location exists, a `related` note/span SHOULD describe the first consume location.

Primary-span requirement:

1. The primary span MUST identify the later consuming operation that encounters
   the already-consumed root.
2. The first consume MUST NOT replace that operation as primary.


### `OWNERSHIP_USE_AFTER_DISCHARGE`

Severity: `error`

Trigger: non-consuming use/borrow (`&x`, `&mut x`, or other value use) references an ownership root, a reference derived from such a root, or a source consumed by `own` that is not definitely available (`Consumed` or `MaybeConsumed`) at that program point.

Message contract summary:

1. `ownership`.
2. `use`.
3. `discharged`.

Data contract summary (if `data` is present):

1. `value`.

Recommended related-span guidance:

1. when a distinct prior discharge or path-merge cause location exists, a `related` note/span SHOULD identify it.

Primary-span requirement:

1. The primary span MUST identify the failing borrow or other non-consuming use.
2. A prior discharge or path-merge cause MUST NOT replace that use as primary.


### `OWNERSHIP_GUARD_CONSUME_FORBIDDEN`

Severity: `error`

Trigger: a `match` guard attempts to consume a tentative ownership binding (the carrier or its payload) through `move`, `drop_own`, or another by-value transfer before the arm is selected.

Message contract summary:

1. `ownership`.
2. `guard`.
3. `consume`.
4. `forbidden`.

Data contract summary (if `data` is present):

1. `value`.


### `STRUCT_INIT_UNKNOWN_FIELD`

Severity: `error`

Trigger: record initializer includes a field name not declared on the target:

1. struct literal target, or
2. enum named-payload constructor target.

Message contract summary:

1. `struct`.
2. `field`.
3. `unknown`.

Data contract summary (if `data` is present):

1. `struct`.
2. `field`.


### `STRUCT_INIT_DUPLICATE_FIELD`

Severity: `error`

Trigger: record initializer assigns the same field more than once:

1. struct literal target, or
2. enum named-payload constructor target.

Message contract summary:

1. `struct`.
2. `field`.
3. `duplicate`.

Data contract summary (if `data` is present):

1. `struct`.
2. `field`.


### `STRUCT_INIT_MISSING_FIELD`

Severity: `error`

Trigger: record initializer omits one or more declared fields:

1. struct literal target, or
2. enum named-payload constructor target.

Message contract summary:

1. `struct`.
2. `field`.
3. `missing`.

Data contract summary (if `data` is present):

1. `struct`.
2. `field`.


### `STRUCT_INIT_PROPERTY_FORBIDDEN`

Severity: `error`

Trigger: struct literal initializer uses a declared property name rather than a stored field name.

Message contract summary:

1. `struct`.
2. `property`.
3. `initializer`.
4. `forbidden`.

Data contract summary (if `data` is present):

1. `struct`.
2. `property`.


### `PROP_ACCESSOR_MISSING`

Severity: `error`

Trigger: property declaration omits one or more required accessor methods.

Message contract summary:

1. `property`.
2. `accessor`.
3. `missing`.

Data contract summary (if `data` is present):

1. `property`.
2. `accessor`.


### `PROP_ACCESSOR_SIGNATURE_MISMATCH`

Severity: `error`

Trigger: property declaration contains an accessor whose name or signature does not exactly match the required property contract.

Message contract summary:

1. `property`.
2. `accessor`.
3. `signature`.
4. `mismatch`.

Data contract summary:

1. `property`.
2. `accessor`.
3. `reason`: `signature_mismatch` or `unexpected_accessor`.
4. `actual_signature`.
5. For `signature_mismatch`, `expected_signature` is the required accessor
   signature.
6. For `unexpected_accessor`, `candidates` contains the property's allowed
   accessor signatures in getter-then-setter order.


### `PROPERTY_ASSIGN_FORBIDDEN`

Severity: `error`

Trigger: assignment syntax targets a declared property name instead of a stored field.

Message contract summary:

1. `property`.
2. `assignment`.
3. `forbidden`.

Data contract summary (if `data` is present):

1. `property`.


### `PROGRAM_ENTRY_MISSING`

Severity: `error`

Trigger: an executable invocation's designated entry source contains no
top-level `main!` declaration after ordinary source compilation succeeds.
A non-bang `main` declaration does not satisfy this requirement.

The primary span MUST be the zero-width physical byte range `0..0` in the
designated entry source, with a text projection beginning and ending at line 1,
column 1. The source is valid UTF-8 because this check runs only after ordinary
source compilation succeeds; no declaration owns the failure.

Message contract summary:

1. `program`.
2. `entry`.
3. `missing`.


### `PROGRAM_ENTRY_DUPLICATE`

Severity: `error`

Trigger: an executable invocation's designated entry source contains more than
one top-level `main!` declaration after ordinary source compilation succeeds.

Message contract summary:

1. `program`.
2. `entry`.
3. `duplicate`.


### `PROGRAM_ENTRY_SIGNATURE_INVALID`

Severity: `error`

Trigger: the sole top-level `main!` in an executable invocation's designated
entry source has parameters, has generic parameters, or has a normalized return
type outside the `Unit` / ownership-free and reference-free `Res[T, E]` /
`Never` contract in `../language/program-model.md`.

Message contract summary:

1. `program`.
2. `entry`.
3. `signature`.
4. `invalid`.


### `RESERVED_BUILTIN_NAME`

Severity: `error`

Trigger: a user declaration introduces a reserved built-in or contextual symbol
in v0.x: `Opt`, `Res`, `Own`, `Box`, `Fn`, `Union`, `Never`, or `Self` in the
type namespace, or `Some`, `None`, `Ok`, or `Err` as a value constructor.

Type-namespace introduction includes nominal and trait declarations, type aliases, generic type parameters, and type import aliases.

For the contextual `Self` spelling, this declaration-site diagnostic precedes `SELF_TYPE_INVALID` errors that would arise from uses under the invalid binding.

Message contract summary:

1. `reserved`.
2. `built-in`.
3. Offending symbol name.


### `TYPE_RECURSIVE_LAYOUT_REQUIRES_INDIRECTION`

Severity: `error`

Trigger: the alias-expanded inline-layout graph for user `struct`/`enum` declarations contains a cycle that does not cross a recognized indirection boundary.

Generic substitutions are analyzed without enumerating monomorphized instances; a repeated nominal declaration head on one substitution-aware inline path is a cycle even when its type arguments change.

Message contract summary:

1. `type`.
2. `recursive`.
3. `layout`.
4. `indirection`.
5. At least one type name from the selected cycle edge.

Primary-span requirement and related guidance:

1. the primary span MUST identify the earliest source-positioned storage occurrence belonging to an invalid cycle,
2. when emitted, related text SHOULD identify a deterministic cycle path beginning with that edge using `Owner -> Target` notation.


### `TYPE_ARG_INFERENCE_FAILED`

Severity: `error`

Trigger: omitted generic type arguments cannot be inferred from available argument, nominal-constructor field/payload, and expected-type constraints; nominal-constructor constraints are inconsistent or admit no unique complete owner vector; a generic function-value expression lacks a complete expected `Fn` type; or a call supplies only a partial explicit type-argument list where v0.1 requires all-or-none explicit arguments. Matching an ordinary concrete type against an associated projection such as `T::Item` is not an available constraint for inferring `T`.

Message contract summary:

1. `type argument`.
2. `inference`.
3. `failed`.

Recommended help guidance:

1. add expected type context, including an annotated binding for a generic
   nominal constructor, or
2. for an immediate generic call, provide explicit type arguments using
   `::[...]`.


### `TYPE_ARG_ARITY_MISMATCH`

Severity: `error`

Trigger: a generic type path or type alias use supplies the wrong number of type
arguments for the referenced type constructor or alias, or a zero-arity type
such as `Never` or an associated type is followed by type arguments such as
`Never[T]`, `T::Item[U]`, or in-scope `Self::Item[U]`.

Message contract summary:

1. `type argument`.
2. `arity`.
3. Expected and actual argument counts.

Data contract summary:

1. `expected_count`: the referenced declaration's generic arity.
2. `actual_count`: the number of supplied type arguments.

Recommended help guidance:

1. provide exactly the declared number of type arguments.


### `TYPE_ARG_BRACKET_SYNTAX_FORBIDDEN`

Severity: `error`

Trigger: an ordinary generic-call attempt uses bracket-only call-site type-argument syntax such as `f[T](x)` instead of the v0.1 `f::[T](x)` surface.

This code does not apply to dedicated tensor constructor dtype-bracket forms such as `ones[f32](...)`, which are specified separately by `stdlib/tensor.md`.

Message contract summary:

1. `type argument`.
2. `bracket`.
3. `forbidden`.

Recommended help guidance:

1. use `::[...]` for explicit call-site type arguments.


### `NUMERIC_OPERAND_NOT_NUMERIC`

Severity: `error`

Trigger: unary numeric negation (`-`) receives a non-numeric operand, or a
binary numeric arithmetic operator (`+`, `-`, `*`, `/`, `/.`, `%`) receives at
least one non-numeric operand type.

Message contract summary:

1. `numeric`.
2. `operand`.
3. Operator token (for example `+` or `*`).
4. Offending operand type name (for example `Tri`).

Data contract summary:

1. `reason`: `unary_operand` or `binary_operand`.
2. `operation`.
3. For `unary_operand`, `actual_type` is the canonical operand type.
4. For `binary_operand`, `left_type` and `right_type` are the canonical operand
   types.

Binary operand types are the independently typed source operands before
numeric promotion; a valid numeric counterpart retains its own canonical type.


### `NUMERIC_CHECKED_CONTEXT_REQUIRED`

Severity: `error`

Trigger: a potentially faulting scalar fixed-width integer operator is used outside an active lexical `checked { ... }` boundary and is not one of the statically total forms defined by `language/type-system.md`.

Message contract summary:

1. `numeric`.
2. `checked`.
3. Operator token (for example `+` or `/`).

Recommended help guidance:

1. wrap the arithmetic region in `checked { ... }` and handle the resulting `Res`;
2. use an explicit scalar wrapping function when modular arithmetic is intended.


### `LOGICAL_OPERAND_TYPE_MISMATCH`

Severity: `error`

Trigger: logical operator (`&&` or `||`) receives an operand whose type is neither `Bool` nor `Tri`.

Message contract summary:

1. `logical`.
2. `operand`.
3. Operator token (`&&` or `||`).
4. `Bool`.
5. `Tri`.
6. At least one offending operand type name.

Data contract summary:

1. `operation`.
2. `left_type`.
3. `right_type`.

Recommended help guidance:

1. use `Bool` / `Tri` operands or convert explicitly before applying the logical operator.


### `ABSTRACT_NUMERIC_KIND_VALUE_POSITION`

Severity: `error`

Trigger: abstract numeric umbrella kind (`Int`, `UInt`, `Integer`, `Float`, or `Complex`) appears where v0.1 requires a concrete value dtype, including parameter types, return types, local or field annotations, and generic concrete arguments.

Message contract summary:

1. `abstract`.
2. `numeric`.
3. `kind`.
4. `value`.
5. `position`.

Data contract summary (if `data` is present):

1. `kind`.
2. `position`.

Recommended help guidance:

1. use a fixed-width concrete dtype such as `i64`, `f64`, or `complex128`.


### `COMPARISON_TYPE_MISMATCH`

Severity: `error`

Trigger: equality (`==`, `!=`) or relational (`<`, `<=`, `>`, `>=`) operator operands fail comparison typing rules, including:

1. non-numeric implicit coercion or operand-type mismatch for relational operators,
2. non-numeric equality mismatch,
3. generic argument mismatch such as `Opt[T]` vs `Opt[U]` where `T != U`,
4. equality on a same-static-type operand pair whose type does not implement the canonical `std::prelude::Eq`,
5. equality in generic code without an applicable `Eq` bound,
6. equality on a conditional carrier whose required nested `Eq` obligation is not satisfied,
7. equality on a type whose defining contract explicitly excludes observable equality, such as `Task[T]`, `Own[T]`, a reference, or a callable value,
8. relational comparison on a same-static-type operand pair whose type does not implement the canonical `std::prelude::Ord`,
9. relational comparison in generic code without an applicable `Ord` bound,
10. numeric promotion of relational operands to `complex64` or `complex128`, which do not implement `Ord`.

Message contract summary:

1. `comparison`.
2. `type`.
3. Operator token (`==`, `!=`, `<`, `<=`, `>`, or `>=`).
4. At least one operand type name.

Data contract summary:

1. `operation`.
2. `left_type`.
3. `right_type`.

The two fields are the operand types presented to the comparison rule after any
permitted numeric promotion. They may therefore be equal when the failure is a
missing `Eq` or `Ord` obligation rather than operand inequality.
