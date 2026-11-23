# Control Flow

Scope: Normative for v0.1 unless this file or a section states otherwise.

### Conditional Expressions

`if` is an expression form in Eglur v0.1.
It produces a value only when the selected branch produces an explicit block result.
Its condition uses the `ControlHeadExpr` surface and brace boundary defined in
[`syntax.md`](syntax.md#control-head-brace-boundary).

Canonical forms:

```text
if <cond_bool> { yield <expr_a> } else { yield <expr_b> }

if <cond_tri> { yield <expr_true> } else { yield <expr_false> } unknown { yield <expr_unknown> }
```

Rules:

1. `<cond>` MUST have type `Bool` or `Tri` (`IF_CONDITION_NOT_BOOL` otherwise).
2. If `<cond>` has type `Bool`, an `unknown { ... }` arm MUST NOT be present (`IF_BOOL_UNKNOWN_BRANCH_FOR_BOOL` otherwise).
3. If `<cond>` has type `Tri`, an `unknown { ... }` arm MUST be present (`IF_TRI_UNKNOWN_BRANCH_REQUIRED` otherwise).
4. Bool-form branches (`if`, `else`) MUST type-check to a single common result type (`IF_BRANCH_TYPE_MISMATCH` otherwise).
5. Tri-form branches (`if`, `else`, `unknown`) MUST type-check to a single common result type (`IF_BRANCH_TYPE_MISMATCH` otherwise).
6. `<cond>` MUST be evaluated exactly once.
7. Only the selected branch MAY be evaluated.
8. `if` without `else` is not a value-producing expression in v0.1 and MUST be rejected where a value is required (`IF_MISSING_ELSE_VALUE`).
9. Branch bodies are ordinary `BlockExpr` forms:
   1. a branch that contributes a non-`Unit` value MUST use `yield`,
   2. a trailing branch expression without `yield` is an `ExprStmt`; its value is
      discarded, and a reachable normal branch completion contributes `Unit`,
   3. no implicit tail-expression result rule is introduced for `if` branches.
10. Common-result selection in Rules 4-5 ignores a branch evaluation with no
    normal continuation; an exact-`Never` branch necessarily has that fact. If
    no branch has a normal result edge, the `if` result type is `Never`. Every
    branch remains subject to ordinary source validation even when another
    branch or the condition has no normal continuation.

`else if` is syntactic sugar and MUST desugar to nested `if` in the `else` branch:

```text
if c1 { yield a } else if c2 { yield b } else { yield c }
```

desugars to:

```text
if c1 { yield a } else { yield if c2 { yield b } else { yield c } }
```

`else if` remains valid syntax. Each desugared nested `if` is independently checked under Bool/Tri rules above.
This means a nested `else if` whose condition has type `Tri` still requires its own `unknown` branch.

### `match` Expressions

`match` is a value-producing expression in v0.1.
Its scrutinee uses the `ControlHeadExpr` surface and brace boundary defined in
[`syntax.md`](syntax.md#control-head-brace-boundary).

Canonical form:

```text
match <scrutinee> {
  <pattern_1> [if <guard_1>] => <expr_1>,
  ...
  <pattern_n> [if <guard_n>] => <expr_n>,
}
```

Rules:

1. Scrutinee expression MUST be evaluated exactly once.
2. Arms are tested in source order, top-to-bottom.
3. The first arm whose pattern matches and whose guard (if present) evaluates to `true` is selected.
4. Except for the valid zero-arm case in Rule 14, if no arm can match,
   compilation MUST fail with `MATCH_NON_EXHAUSTIVE`.
5. All arm result expressions with a normal continuation MUST type-check to a
   single common result type (`MATCH_ARM_TYPE_MISMATCH` otherwise). An arm with
   no normal result edge does not select or widen that type; if every arm lacks
   one, the result is `Never`.
6. Guards MUST be pure expressions in v0.1; effectful guard expressions MUST fail with `MATCH_GUARD_EFFECT`.
7. Guard expressions MUST type-check to `Bool` (`MATCH_GUARD_TYPE_MISMATCH` otherwise).
   1. `Tri` guards are not implicitly accepted in v0.1.
   2. Code that wants three-valued guard semantics MUST convert to `Bool` explicitly (for example by comparing with `Tri::True` or by using a documented `Tri` helper such as `coalesce`).
8. Guarded arms do not establish unconditional coverage for exhaustiveness:
   1. for v0.1 coverage analysis, an arm with `if <guard>` MUST be treated as partial coverage only,
   2. implementations MUST NOT consider guard truth as statically guaranteed, even for literal guards.
9. An arm proven unreachable because one or more earlier arms already cover all remaining cases MUST fail with `MATCH_UNREACHABLE_ARM`.
10. For an or-pattern `p1 | p2 | ...`, every alternative MUST introduce the same binding-name set, and each same-named binding MUST have the same inferred binding type (`MATCH_BINDING_MISMATCH` otherwise).
    Diagnostic selection compares each later alternative with the first in
    source order. For the first failing alternative, a binding-set difference
    precedes a binding-type difference; the selected binding is the first
    differing name under canonical lexical text order.
11. The valid bindings introduced by one arm pattern form an arm scope after the
    pattern matches. They are active in that arm's guard and result expression,
    but not in sibling arms or after the `match`. Ownership-bearing bindings are
    tentative during guard evaluation as specified below.
12. Each arm has a distinct arm scope. If its result expression is a
    `BlockExpr`, that block introduces a nested local scope and MAY shadow an arm
    binding under `bindings-structs.md`.
13. An arm body is an `Expr`. If that expression is a `BlockExpr`, it follows the same explicit block-result rules as other blocks:
    1. a block arm that contributes a non-`Unit` value MUST use `yield`,
    2. trailing expressions in block arms are discarded when not written as `yield <expr>`.
14. A match with zero arms is valid exactly when the alias-expanded scrutinee
    type is `Uninhabited` under the bounded analysis in `type-system.md`. Its
    result type is `Never`. An `Inhabited` or `Unknown` scrutinee instead fails
    with `MATCH_NON_EXHAUSTIVE`.
15. A variant whose complete payload product is `Uninhabited` contributes no
    required coverage. An arm that can match only impossible variants fails with
    `MATCH_UNREACHABLE_ARM`.
16. Exhaustiveness and reachability use structural inhabitance only for the
    closed decisions in Rules 14-15. They MUST NOT rewrite the scrutinee type to
    `Never`, infer a trait, or add a general uninhabited-pattern calculus.
17. Pattern, guard, arm-expression, effect, and ownership validation still runs
    for every written arm before coverage and reachability diagnostics under the
    precedence below.

v0.1 required pattern set:

1. Wildcard pattern: `_`.
2. Literal patterns (numeric, boolean, string).
3. Unit pattern: `()`.
4. Binding patterns: `name`.
5. Alias binding: `name @ pattern`.
6. Variant patterns for algebraic data (for example `Flag::On`,
   `Token::Number(value)`, `Some(x)`, `None`, `Ok(v)`, and `Err(e)`).
7. Tuple patterns.
8. Struct/record patterns by field name.
9. Or-patterns: `p1 | p2`.
10. Rest pattern in tuple/struct/variant positional contexts: `..`.
11. Nested composition of the above pattern forms.

`..` is rest token in pattern contexts. In expression contexts, `..` and `..=` are range operators. Range-pattern forms remain out of scope for v0.1.

Pattern name resolution:

1. Pattern name resolution is deterministic and occurs after the scrutinee type is known and before exhaustiveness, reachability, and binding-set checks.
2. Source enum and catalog-sum variant patterns MUST use the owner path
   (`Owner::Variant`, `Owner::Variant(...)`, or `Owner::Variant { ... }`).
3. Built-in residual constructors (`Some`, `None`, `Ok`, `Err`) remain the only unqualified constructor pattern heads in v0.1.
4. A bare unqualified identifier pattern that is not a built-in residual
   constructor introduces a new binding; it does not look up an existing value,
   type, const, or nominal-sum variant.
5. If such a bare identifier has the same spelling as a variant of the
   scrutinee's nominal sum type, compilation MUST fail with `NAME_AMBIGUOUS`;
   write the qualified `Owner::Variant` pattern to match the variant, or choose
   a different binding name to bind the whole value.
6. A braced pattern with an explicit path is classified by the resolved
   declaration kind: `StructName { ... }` is a struct pattern, while
   `Owner::Variant { ... }` is a named-payload variant pattern. Neither form is
   a binding pattern.
7. Pathless struct patterns (`{ field, ... }`) use the known scrutinee struct
   type; they do not infer variants.

Structural pattern validation:

1. Every pattern node is validated against one expected type. The match
   scrutinee establishes the root expected type; tuple elements, struct fields,
   and variant payload entries establish the expected types of nested nodes.
2. Pattern-head name resolution and nominal identity checking occur before
   payload validation:
   1. an unresolved or ambiguous head retains the applicable name-resolution
      diagnostic,
   2. a tuple pattern requires a tuple expected type,
   3. `()` requires expected type `Unit`,
   4. a struct pattern requires the same normalized nominal struct type as its
      resolved or context-provided target,
   5. a variant pattern requires the same normalized nominal-sum owner as its
      resolved variant,
   6. a non-interpolated literal pattern first undergoes ordinary contextual
      literal validation, preserving a more specific cause diagnostic such as
      `INT_OVERFLOW` or `DOMAIN_INVALID_VALUE`; after that succeeds, an
      incompatible literal kind/type MUST use `PATTERN_TYPE_MISMATCH`.
   A failure in Rules 2.2-2.5 MUST use `PATTERN_TYPE_MISMATCH`.
3. `_` and a binding pattern accept any expected type. An alias pattern
   validates its nested pattern against that same expected type. Binding-name
   validation occurs only after the complete structural walk succeeds.
4. Tuple patterns are exact-arity by default:
   1. without `..`, the written item count MUST equal the tuple arity,
   2. with a tail `..`, the explicit prefix count MUST NOT exceed the tuple
      arity and `..` matches every remaining element, including zero elements,
   3. `(..)` matches every tuple arity but does not match `Unit`.
   An item-count failure MUST use `PATTERN_ARITY_MISMATCH`.
5. A resolved variant pattern MUST use the payload form declared by that
   variant:
   1. a unit variant uses the bare `Owner::Variant` form,
   2. a positional variant uses `Owner::Variant(...)`, including when its
      declared positional arity is zero,
   3. a named-payload variant uses `Owner::Variant { ... }`.
   Bare payload-bearing variants do not implicitly discard their payload. A
   form mismatch MUST use `PATTERN_PAYLOAD_FORM_MISMATCH`.
6. Positional variant payloads use the tuple arity/rest rules from Rule 4.
   Their item-count failures MUST use `PATTERN_ARITY_MISMATCH`.
7. Struct and named-payload variant patterns are exact-field by default:
   1. every written field name MUST occur at most once
      (`PATTERN_DUPLICATE_FIELD` otherwise),
   2. every written field name MUST name a declared field of the target
      (`PATTERN_UNKNOWN_FIELD` otherwise),
   3. without `..`, every declared field MUST be written
      (`PATTERN_MISSING_FIELD` otherwise),
   4. one `..` explicitly permits any otherwise omitted declared fields,
   5. field order is not semantically significant,
   6. shorthand `field` is equivalent to `field: field` only after the field
      name has passed Rules 7.1-7.2.
8. After a container's form, arity, and field set are valid, its explicit child
   patterns are validated recursively in source order against their declared
   element or field types.
9. After the complete structural walk succeeds, binding validation proceeds:
   1. on every complete alternative path through nested or-patterns, one
      binding spelling MUST be introduced at most once
      (`PATTERN_DUPLICATE_BINDING` otherwise),
   2. an alias binding has the expected type of its whole nested pattern,
   3. each or-pattern is then checked recursively under Rule 10 of the match
      rules for identical binding-name sets and same-named binding types.
10. Structural validation is deterministic and uses this primary-failure
    precedence:
    1. parser and pattern-head name-resolution failures,
    2. contextual literal validity and its specific cause diagnostic,
    3. outer type/nominal-identity mismatch,
    4. variant payload-form mismatch,
    5. tuple or positional-payload arity mismatch,
    6. duplicate, then unknown, then missing named fields,
    7. the first invalid child in source-order depth-first traversal,
    8. duplicate binding on the first complete alternative path in source
       order,
    9. or-pattern binding-set/type mismatch,
    10. overlapping ownership bindings on one complete alternative of a
        conditional-root pattern (`PATTERN_OWNERSHIP_BINDING_OVERLAP`),
    11. exhaustiveness and reachability analysis.
    Within Rules 10.6 and 10.8, the later occurrence of the first duplicate in
    source order is primary. For a missing field, the first omitted declaration
    field is primary and the enclosing explicit pattern head, or the opening
    `{` of a pathless struct pattern, supplies the primary source span because
    no field occurrence exists.

Examples:

```eglur
Flag::On => 1  // user enum variant pattern
Tri::Unknown => 2  // catalog-sum variant pattern
On => 1        // ambiguous when the scrutinee type has a variant named On
x => x         // binding pattern when there is no scrutinee-variant collision
```

When ownership surface is enabled, `match` ownership-path analysis MUST additionally satisfy:

1. guard-true and guard-false path splits are distinct typed paths for ownership accounting,
2. typed exits from selected arm expressions are included in ownership discharge analysis,
3. function-level typed exits through every arm are included.

Patterns do not consume ownership by themselves in v0.1.
An ownership-bearing scrutinee must be transferred before decomposition: use `match move r { ... }` for a named conditional root.
The selected arm can bind the whole carrier or its owned payload, as specified
in [Conditional Root Carriers](ownership-boundary.md#conditional-root-carriers).
During guard evaluation the binding is tentative: only a non-escaping shared
borrow is permitted (`OWNERSHIP_GUARD_CONSUME_FORBIDDEN` on a consuming use).
If the guard fails, the moved carrier remains available for later arms.


### Block Expressions and `yield`

`yield` is the explicit block-result form in v0.1.
It gives a block expression its value; it is not a function-level early return.

Rules:

1. `yield <expr>` first evaluates `<expr>`. Only normal completion of that
   expression creates an edge that exits the innermost enclosing block and
   carries its value.
2. A reachable normal fallthrough edge contributes `()` of type `Unit` to the
   block result.
3. A trailing expression without `yield` is an `ExprStmt`; its value is discarded
   and it contributes only the normal fallthrough from Rule 2.
4. The block result is the common type of its normal `yield` and fallthrough
   edges. An evaluation carrying the no-normal-continuation fact creates no
   result edge even when expected-type compatibility gives the enclosing
   expression another ordinary type. If no normal result edge exists, the block
   result type is `Never`.
5. Implicit tail-expression block return is not part of v0.1.
6. In function/method block bodies, return value is the enclosing block result (or an earlier `return`), then checked against declared/inferred function return type.
7. `fn ... = <expr>` remains a separate expression-body shorthand from block-body `yield` semantics.
8. For ownership analysis, a `yield` edge exits only the obligation region of
   the block expression identified by Rule 1. Roots belonging to enclosing
   regions remain live unless the yielded expression explicitly consumes them.
9. Every root introduced in the exited block region MUST be discharged or
   transferred through the yielded result (`OWNERSHIP_PATH_UNBALANCED`
   otherwise).
10. A `yield` from the function/method body also completes the callable and
   therefore exits every active obligation region in that callable.
11. A written `yield` whose operand has no normal continuation creates no yield
    edge but remains subject to ordinary source validation. A structurally
    uninhabited operand type other than exact `Never` does not receive this rule.

These rules apply to `BlockExpr` forms.
Closure literals use separate `LambdaBody` syntax and closure-tail expression semantics defined in `calls-references.md`.
That closure-tail rule does not introduce implicit tail-expression return for ordinary block expressions or function/method block bodies.


### `return` Statements

`return` is a statement form for explicit function/method early exit in v0.1.

Canonical forms:

```text
return
return <expr>
```

Rules:

1. `return` MUST appear inside an enclosing function or method body and outside any nested closure body (`RETURN_OUTSIDE_FUNCTION` otherwise).
2. `return <expr>` evaluates `<expr>` and exits the enclosing function/method immediately.
3. Bare `return` exits the enclosing function/method immediately with `()`.
4. Bare `return` is allowed only when enclosing function/method return type is `Unit` (`RETURN_VALUE_REQUIRED` otherwise).
5. `return <expr>` is forbidden when enclosing function/method return type is `Unit` (`RETURN_VALUE_FORBIDDEN` otherwise).
6. For non-`Unit` enclosing return types, type of `<expr>` in `return <expr>` MUST be assignable to declared/inferred return type (`RETURN_TYPE_MISMATCH` otherwise).
7. `return` inside nested blocks, loops, `if`, `match`, or `result {}` exits the enclosing function/method, not just the inner construct.
8. Closure bodies are control-flow boundaries in v0.1:
   1. `return` inside a closure body is forbidden and MUST fail with `RETURN_OUTSIDE_FUNCTION`,
   2. use closure tail expressions to produce closure results.
9. `return` statements are typed-exit points for ownership analysis in `ownership-boundary.md`.

Rules 4-5 are form restrictions that precede bottom compatibility.
`return abort!(...)` therefore remains `RETURN_VALUE_FORBIDDEN` in a `Unit`
function. In a non-`Unit` function, an operand with no normal continuation
creates no return edge because operand evaluation does not complete.


### Iterative Statements

v0.1 defines loop forms as statements, not value-producing expressions.
The `while` condition and `for-in` iterable use the `ControlHeadExpr` surface
and brace boundary defined in
[`syntax.md`](syntax.md#control-head-brace-boundary).

Canonical forms:

```text
while <cond> { <body> }
for <name> in <iterable> { <body> }
for <name> in <start> .. <end> { <body> }
for <name> in <start> ..= <end> { <body> }
break
continue
```

Rules:

1. `while` condition MUST type-check to `Bool` (`LOOP_CONDITION_NOT_BOOL` otherwise). `Tri` conditions are not allowed in `while` in v0.1 and MUST be rejected.
2. `while` condition MUST be evaluated exactly once per iteration attempt, before body execution.
3. `for-in` iterable MUST implement sequence iteration semantics with deterministic encounter order (`FOR_ITERABLE_NOT_SEQUENCE` otherwise).
4. In v0.1, `for-in` binder MUST be a single identifier binding.
5. `break` exits the innermost enclosing loop immediately.
6. `continue` skips to the next iteration of the innermost enclosing loop.
7. Closure bodies are control-flow boundaries for loop targets:
   1. `break` / `continue` inside a closure body may target only loops lexically inside that same closure body,
   2. they MUST NOT target an outer loop surrounding the closure literal.
8. `break` and `continue` outside loop bodies in the same control-flow region MUST be compile-time errors (`BREAK_OUTSIDE_LOOP`, `CONTINUE_OUTSIDE_LOOP`).
9. Loop bodies follow the same effect rules as surrounding function context:
   1. in non-bang functions, effectful calls remain forbidden;
   2. in bang functions, effectful calls are permitted.
10. Range iteration expressions are built-in iterable expressions over `i64` only when they appear as the `for-in` iterable expression:
   1. `<start> .. <end>` is half-open and yields `<start>, <start>+1, ... , <end>-1`.
   2. `<start> ..= <end>` is closed and yields `<start>, <start>+1, ... , <end>`.
11. Range syntax outside a `for-in` iterable expression has no value semantics in v0.1 and MUST fail with `RANGE_EXPR_UNSUPPORTED`.
12. Range bounds in `for-in` MUST type-check to `i64` (`FOR_ITERABLE_NOT_SEQUENCE` otherwise).
13. Range bounds are signed arithmetic endpoints, not `Nat` size/count domains.
14. Range bounds MUST be evaluated exactly once, left-to-right, before loop entry.
15. v0.1 range iteration step is fixed to `+1` and descending ranges execute zero iterations.
16. `for-in` iterable name resolution and evaluation occur outside the loop
    binder scope. An outer same-spelled binding therefore remains visible in the
    iterable expression.
17. The loop binder is initialized once per iteration and is visible only in
    the loop body. It is not visible after the loop.
18. The loop-body `BlockExpr` is nested inside the binder scope and MAY shadow
    the binder under `bindings-structs.md`.
