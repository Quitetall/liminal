# DG17.6 implementation sequence

Status: implementation design after approved DG17.6; not completed code or proof.
Baseline: `9b34d10599ecd1ba26fb334115838f64e5631384`.

## Separation of responsibilities

Keep `topo_order`'s existing DAG contract. In particular, do not add ILRP's
graph-before-external prohibition to that general function: callers currently
validate that rule separately, after other checks. Moving it would change
diagnostic precedence and the meaning of this public function.

One host module owns temporary proof representation. It copies exact map keys,
declared step identities, graph/external classification, dependency endpoints,
and the actual schedules into fallibly allocated storage. Preserve duplicates
and source ordering; do not normalize away a defect before the checker sees it.
Use UUID's defined integer conversion, never layout reinterpretation or an
unsafe cast. These copies describe inputs; construction alone creates no
authorization or trusted repair capability.

The allocation-free leaf receives borrowed slices. Its complete checker must
establish unique complete membership, matching map/declared identities, known
dependency endpoints, strict edge ordering, smallest-ready identity ordering,
and ILRP's forbidden graph-before-external edges. Check the actual applied
schedule's stable external-first partition and finalization's graph projection
against the same identities. No separate handwritten semantic twin qualifies.

## Exhaustion contract

Use checked size arithmetic before allocation. Reserve all required capacity
fallibly before filling buffers; filling must not trigger an implicit growth
allocation. Partially reserved storage drops normally on refusal. No fixed
plan-size limit, enabled vstd allocation feature, or identity-layout change.

`ResourceExhaustion` carries no allocated message or collection. Propagate it
as a distinct variant through admission and ILRP. Current admission at
`admission.rs:151` converts DAG errors to allocated review reasons; Prepare at
`ilrp.rs:601-604` stringifies admission errors. New exhaustion must not flow
through either conversion. Preserve existing mappings for existing errors.

New preparation runs after the existing checks that would otherwise refuse,
and before the next accepted intent/effect/recovery advancement. Its scope is
new proof storage only: existing Kahn, serialization, store, and receipt
allocations are not claimed fallible by this work.

## Test and integration sequence

Confirmed public test seams (user: "Confirm these seams"): checked proof-buffer
preparation, Checker automatic and human admission, and IlrpDriver
Prepare/recovery. Allocation-fault controls must exercise
the allocation/resource seam, not replace the checker with a mock or exhaust
the host machine. Exact fault-injection mechanics remain to be selected.

The user additionally confirmed the direct leaf seam: "Confirm direct leaf
seam". Tests may call the public allocation-free
`liminal_safety::ordering_matches(steps, dependencies, schedule, applied)`
Boolean predicate with malformed raw inputs. This does not replace host,
Checker, or ILRP integration controls and grants no repair authority.

1. One red/green translation control using literal nontrivial 128-bit IDs,
   mismatched declared IDs, duplicate dependencies, and explicitly ordered
   schedules. Invalid input must remain visible to subsequent validation.
2. Checked-size/capacity and allocation-refusal controls at the agreed resource
   seam; error propagation must not allocate a Review vector or Executor string.
3. Complete executable leaf checker and real positive/negative proof controls.
   Translate and validate actual values before relying on any ghost precondition.
4. Integrate after existing validation in admission, Prepare, persisted intent
   validation, Apply/recovery, and finalization/history paths. Assert refused
   preparation leaves existing accepted state unchanged through public reads.
5. Preserve and test existing diagnostics when both a legacy validation defect
   and forced proof-resource refusal are present. Successful behavior remains
   unchanged when resources are available.
6. Audit affected frozen coordinates and source pins, perform required reviewed
   maintenance only, then full CI and later fixed-source qualification.

The two completed cold leaf cycles are historical development evidence. They do
not qualify later source changes or close any whole-core obligation. DG17.6 is
approved; the unresolved items above are implementation/test details, not a
request to approve that decision again.

## Call-site precedence audit (construction slice)

Inspected against baseline `9b34d105` with the uncommitted buffer module;
these are integration constraints, not completed integration evidence.

- `admission.rs:110-118`: automatic admission performs `evaluate_repair` and
  a second head check after `validate_admission_inputs`. Putting the new
  preparation inside that shared input validator would mask existing
  `NeedsReview`, checker errors, or changed-head refusal. Prepare proof storage
  only after these automatic-route checks. Human admission has no automatic
  evaluation; retain that distinction and its caller-supplied actor.
- `ilrp.rs:601-604`: `consume` currently has only legacy admission failures.
  Do not add fallible proof preparation there and then stringify its error.
  Any future resource-error branch must remain typed and nonallocating.
- `ilrp.rs:628-642`: Prepare's duplicate-intent refusal precedes intent
  construction. New proof preparation must not mask that refusal and must
  complete before beginning the durable Prepare transaction.
- `ilrp.rs:713-731`: `advance` commits Applying or Finalizing before reaching
  its current Apply schedule construction at line 738. Preparing new proof
  buffers only at that schedule site is too late: exhaustion could follow
  recovery advancement. Prepare and validate the needed buffers before the
  state-changing match, preserving the illegal-transition refusal first.
- `ilrp.rs:683-705`: recovery is a per-intent loop, not an atomic batch.
  A later intent's refusal must not be described as rolling back earlier
  completed intents. Test no advancement for the refused intent; retain the
  existing batch semantics and already durable work.

The successful `OrderProofInput` constructor alone proves none of these
call-site properties. Public admission/driver fault controls remain required.

## Integration audit after the ordering leaf

At source commit `6331ef7be0a300e9d7ddce54ca8167caf9876e80`, the public leaf
exists and has development proof/runtime controls; production host integration
is still pending. The following constraints refine the integration sequence,
without changing the approved exhaustion contract:

- `ilrp.rs:459-461`: `run_checked` calls `verify_history` after `run` has
  performed effects. Adding fresh fallible proof-buffer construction directly
  to every history check would therefore introduce a new exhaustion refusal
  after effects. The execution path must carry or reuse pre-effect validated
  immutable ordering facts through receipt reconstruction. Standalone history
  validation is read-only, but that does not make its post-run caller pre-effect.
- `ilrp.rs:500-524`: history reconstruction separately derives graph operations
  and the receipt's applied order. Both actual projections must remain bound to
  the checked plan and schedule; checking an unused freshly derived partition
  would not establish this relationship.
- `ilrp.rs:802`: finalization follows external application and acknowledgment.
  Its graph projection must consume or compare against the prechecked facts,
  not allocate another proof input at that point. Existing state, persisted
  intent, graph prestate/poststate and head checks remain mandatory.
- `ilrp.rs:936`: `validate_intent` is used by history and finalization as well
  as initial recovery. It cannot become an unconditional allocating proof
  hook without violating the preceding timing constraints. Keep legacy
  validation precedence explicit at each effect boundary.

These are verified call-path constraints, not claims that integration is done.
The direct allocation harness remains external development evidence: the
workspace forbids unsafe code, while a real `GlobalAlloc` fault injector
requires an unsafe implementation. Do not silently relax that workspace lint
or add a production fault-injection switch to make integration tests convenient.
