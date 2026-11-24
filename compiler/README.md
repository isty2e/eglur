# Eglur Compiler

This Rust workspace implements the Eglur lexer/parser and a bounded subset of
semantic analysis and execution. It is used to check the specification against
working code, not to claim a complete v0.1 implementation. Tensor lowering,
native code generation, and package acquisition are not implemented here.

## Crate Responsibilities

The crates depend on one another as follows:

```text
eglur-driver -> eglur-diagnostics, eglur-eval, eglur-hir, eglur-sema, eglur-syntax, eglur-source
eglur-eval -> eglur-hir, eglur-sema
eglur-sema -> eglur-diagnostics, eglur-hir, eglur-source, eglur-syntax
eglur-hir -> eglur-syntax -> eglur-source
eglur-diagnostics -> eglur-hir, eglur-source
```

`eglur-source` owns file identity, source bytes, and source coordinates.
`eglur-syntax` owns lexing, parsing, recovery, the lossless syntax tree, and
typed syntax access. `eglur-hir` is the sole production lowering boundary. It
owns source-semantic arenas, occurrence IDs, structural invalid regions, and
written or derived source origins. Tensor syntax remains source structure in
HIR; solver, runtime, and backend facts belong to later owners.
`eglur-diagnostics` owns canonical diagnostic records, independent roots,
poison provenance, deterministic retention and ordering, finalized snapshots,
and plain-text presentation. Semantic phases decide whether a failure is an
independent root; the store prevents derivative checks from emitting another
diagnostic. Parser-local diagnostics remain owned by `eglur-syntax` until the
driver normalizes them at ingress.
`eglur-sema` owns declarations, canonical types and signatures, checked trait
facts, and body typing. Trait processing has two boundaries: signature
resolution collects every impl header and checks authority and coherence before
it inspects associated bindings; the following trait-environment phase builds
the bounded lookup index and memoized projection normalizer. Body typing may
query that environment after local substitution, but it does not search impls,
infer backward through associated bindings, or retype a body.
Source and trusted traits both contribute explicit canonical method membership;
validated impls populate one exact method-target map used by body typing,
effect analysis, preflight, and execution. Functional adapters such as
`to_str` follow checked trait evidence into that map rather than rediscovering
an implementation from names or runtime values.
`eglur-eval` owns bounded execution of sealed `CheckedProgram` values. It
preflights the complete program before execution and reports unsupported
checked operations through typed reasons rather than partial interpretation.
`eglur-driver` owns canonical in-memory file-set requests, session-local source
identity, source-to-module mapping, parser/HIR orchestration, diagnostic
ingress, and import-SCC scheduling over already-resolved `FileId` edges. It
does not acquire packages or resolve import paths. A mapped `.eglur` path owns
module and file identity. Callers may provide a separate, unique source label
for diagnostics; changing that label does not change semantic fingerprints.

The concrete tree library is private to `eglur-syntax`. Semantic phases consume
`HirFile`, not raw tree nodes, and lowering never re-lexes or reparses source.
Trusted enum metadata uses the syntax crate only to validate identifier spellings.

## Parser Failure Boundary

The production parser pre-indexes delimiter pairs in one source-order pass and
uses one checked linear budget for parser and marker events. Recursive grammar
depth is capped at 256, matching the portable v0.1 diagnostic contract. Event
and delimiter exhaustion report the structured
`PARSE_RESOURCE_LIMIT_EXCEEDED` cause rather than a grammar error.

Compilation-unit recovery is deliberately conservative. After one syntax root,
the parser resumes only at a declaration starter on a new logical line whose
delimiter depth is known to be zero. The failed declaration and any skipped
tokens remain in `ErrorNode` regions. Each diagnostic retains that recovery
region so HIR invalid occurrences can carry exact syntax-cause provenance. If
delimiter trust is lost, no such
boundary exists, or the deterministic diagnostic cap is reached, the parser
drains the rest of the file instead of guessing. Expression-only test entry
points remain fail-fast.

Any terminal parser-resource failure discards partial events and builds one
canonical `Root/ErrorNode` tree over the complete token stream. Validated input
therefore remains byte-for-byte reconstructable on every syntax failure path;
allocator or operating-system failure is outside this in-process guarantee.

## Toolchain

Use a current stable Rust toolchain with Cargo, rustfmt, and Clippy. The project
does not pin a compiler release: edition 2024 defines the language baseline,
and `Cargo.lock` fixes the crate graph. Build and benchmark evidence must record
`rustc --version --verbose` so results remain attributable to the compiler that
produced them.

Cargo uses the platform linker. macOS therefore requires Xcode Command Line
Tools; Linux requires a working system C linker. Pixi owns the Python
specification environment and has no role in selecting or invoking Rust.

```bash
rustc --version --verbose
cargo fmt --all -- --check
cargo clippy --workspace --locked --all-targets --all-features -- -D warnings
cargo test --workspace --locked --all-targets
cargo build --workspace --locked
```

## Semantic Integration Tests

The core profile evaluates fixed-width integer expressions and top-level
integer constants, recording their exact values for execution. Constant
dependencies are checked for cycles and same-module forward references before
evaluation; function bodies may refer to later constants. Arithmetic failures remain
compile errors, including inside `checked`. Dynamic arithmetic retains its
`checked` boundary, with the specified total division and remainder exceptions.
Mixed fixed-width integer operands use the shared numeric promotion rules;
conversion overflow is checked before arithmetic. Total division and remainder
exceptions apply only when the operand conversions are also statically safe.
The scalar `wrapping_add`, `wrapping_sub`, and `wrapping_mul` helpers accept
same-dtype fixed-width integers and wrap modulo `2^bits` without `checked`.
Floating/complex arithmetic and Bool numeric operators remain unsupported.
Arithmetic with known nonnumeric operands reports `NUMERIC_OPERAND_NOT_NUMERIC`,
including unary minus and fractional division. Diagnostics retain canonical
operand types before promotion and suppress errors derived from poisoned operands.
Constant initializers cannot execute general function calls.

Source and catalog enums use the same canonical variant and payload declarations.
Bare match bindings are checked against the expected enum's variant names,
including through type aliases and nested patterns. Catalog payloads also
determine enum inhabitance. External variant construction and qualified external
variant patterns remain unsupported.

The ownership pass checks straight-line code, `if` branches, `result` and
`checked` exits, and `match` on ordinary values and owned `Opt` or `Res`
carriers. Different arms can move the same outer root: each arm starts after
the scrutinee. Only arms that continue contribute to the state after the match.
A consuming pattern binds an owned value only after its guard succeeds. If
the guard returns or propagates an error out of the match, the checker
verifies that it cannot leave an owned payload behind.

If a guard always exits the match, values that miss its pattern can still
reach later arms. If a guard can return `false` after changing an outer root
or reference, the checker reports `typecheck.complete_ownership_flow` for
that callable. An incomplete body prevents the program from sealing, but
checked siblings keep their diagnostics.
Loops and closures stop earlier, during body typing.

`SupportProfile::tensor_constructors_v1()` adds static checks for positional
`zeros[T](...)`, `ones[T](...)`, `nan[f32](...)`, and `rand![f32](...)` calls.
It resolves dtype aliases, checks dimensions against `Nat`, and records the
`Tensor[T]` result and call effects. Dynamic dimensions do not require shape
inference. The evaluator rejects these calls before executing the program.
Direct negative integer literals share the core range checks: `-0` is a valid
`Nat`, while negative nonzero dimensions report `DOMAIN_INVALID_VALUE`.
Other dimension expressions still require supported core operations.
Ordinary-call constructor forms, named dimensions, tensor operations, and
storage or backend implementation remain unsupported; the core profile does
not enable constructor checks.

The end-to-end tests read the 23-fixture corpus in
[`core-v1.expectations.json`](../tests/semantic/core-v1.expectations.json),
runs each fixture through `FileSetRequest`, `CompilerSession`, the selected
semantic phases, `CompileAttempt`, and the typed evaluator, then compares the
result with
[`core-v1.observed.json`](../tests/semantic/core-v1.observed.json). Successful
fixtures check values, process status, value diagnostics, and ordered effects.
Rejected fixtures check the portable diagnostic code, data, primary span, and
human/machine projection parity.

The evaluator keeps source tuples in a recursive internal runtime value so a
local tuple may copy reference places without losing referent identity. Public
results, constructors, and runtime hooks cross one recursive projection into
the reference-free `Value` model; that projection rejects any nested place.
Reference-bearing aggregate shapes other than the supported local tuple form
remain `reference_aggregate` preflight refusals.

The semantic-core support profile is compiler-owned. Its support manifest and
machine-readable unsupported inventory are derived from the closed compile,
execution, and evaluator-refusal vocabularies. Fixtures do not select compiler
capabilities. Five separate probe groups cover import SCCs, recursive effect
SCCs, diagnostic roots and poison, support/fingerprint properties, and tensor
non-interference.

Profile selection and catalog coverage are separate. For trusted inherent
methods, the development attempt's capability trace records unavailable owner
inventories with their canonical identities and demand locations. A missing
capability or required input prevents a conclusive source-language result. The
attempt reports `Unsupported` unless a tool-limit failure takes precedence.

Value-member reads resolve stored fields and properties separately from method
calls. Missing source members report `MEMBER_NOT_FOUND`. Trusted nominal
signatures currently omit field/property inventories, so those reads record
`trusted_value_member_inventory` under `typecheck.members` and remain
`Unsupported` rather than treating absent metadata as proof of an absent member.

Catalog-declared nominal, primitive, and intrinsic inherent families support
qualified calls, receiver calls, and pure ownership-free named `Fn` values.
Owner type parameters and
explicit method type arguments remain separate. Associated members have no
receiver and use the owner's qualified name. Complete empty inventories allow
a missing method diagnostic. Execution still requires a compatible runtime
hook and supported argument values; resolving a method does not make opaque
values executable.

The built-in catalog supplies complete primitive inventories:
`Bool`, `Str`, `Nat`, and `PositiveNat` have declared inherent methods; other
primitive inventories are empty. Primitive methods have no execution hooks
yet. `Own`, `Box`, `Opt`, and `Res` preserve their intrinsic identity through
prelude imports and re-exports. Their inherent inventories are not supplied by
the built-in catalog, so inherent lookup reports missing compile input rather
than a missing method. An embedding can supply admitted intrinsic inventories;
this does not add runtime carriers or forward methods from their payload types.

The supporting `Tri`, `Seq`, and `DomainError` declarations are available for
type checking. External enum variant paths and their execution remain
unsupported.

Run the frozen corpus and all five probe groups with:

```bash
compiler/tools/verify_semantic_slice.sh verify
```

To capture a run, write the single JSON report to a file outside the checkout:

```bash
compiler/tools/verify_semantic_slice.sh report > /tmp/eglur-semantic-slice.json
```

The report records the full Git revision and dirty state, Rust toolchain and
host, Cargo profile and lockfile identity, corpus identity, support boundary,
and per-phase observations. Durations are observations for that run, not
throughput promises. The script writes Cargo logs to standard error and emits
JSON on standard output only after every probe succeeds.

An intentional expectation update uses the same production path:

```bash
compiler/tools/verify_semantic_slice.sh snapshot > tests/semantic/core-v1.observed.json
```

The Rust package graph is enforced by
`tests/conformance/tools/test_compiler_package_topology.py` as part of the Pixi
tooling gate. This slice does not provide v0.1 completeness, implementation
conformance, tensor lowering or execution, native code generation, package
acquisition, or persistent caching. The separate implementation-conformance
gate therefore remains [unavailable](../tests/conformance/RELEASE.md).

## Front-End Measurement

The fixed-corpus benchmark harness is private test code and does not add an
Eglur benchmark language surface. Production sessions separately expose
opt-in phase durations and input/output counts for lexer, parser-event, tree,
HIR-lowering, and import-SCC work; ordinary sessions do not read the clock. Run
the fixed-corpus harness from a clean committed revision with the checked-in
corpus and lockfile:

```bash
compiler/tools/measure_frontend.sh timing > /tmp/eglur-frontend-timing.json
compiler/tools/measure_frontend.sh allocation > /tmp/eglur-frontend-allocation.json
```

The script writes Cargo's test log to standard error and exactly one JSON
report to standard output. It always uses a locked release build, one test
thread, the fixed warmup and sample counts in the measurement module, and no
outlier filtering. Timing and allocation are separate invocations: timing runs
without allocator instrumentation, while allocation uses the exact-pinned
`stats_alloc` development dependency over the system allocator.

Each corpus run measures three construction phases:

1. lexer: every UTF-8-valid source, excluding source validation and output
   destruction after the phase;
2. parser events: lex-success non-token sources, excluding lexing, tree build,
   and output destruction after the phase;
3. tree build: the parser input set, excluding lexing, parser-event creation,
   and output destruction after the phase.

Reports retain every raw sample and give nearest-rank median and p95 summaries.
They also bind the corpus and lockfile object IDs, corpus input digest, Git
revision and dirty state, Rust compiler command and host target, Cargo profile,
and available host metadata. Allocation reports contain `stats_alloc`
operation and byte counters only; phase-internal temporary destruction and
consumed-input deallocation remain in their owning phase. The reports do not
claim peak memory or process RSS.

These reports are revision-comparison evidence for this fixed workload. They
are not source throughput, a release performance promise, or a stable public
benchmark schema. Capture reports outside the checkout so output redirection
does not make an otherwise clean revision appear dirty. Compare timing and
allocation reports as a pair only when both runs identify the same clean
revision, corpus, and lockfile.
