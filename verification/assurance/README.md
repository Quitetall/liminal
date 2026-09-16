# Maintaining assurance

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md).
Delivery status: [PLAN.md](PLAN.md). This catalog is classification metadata, not
a coverage proof, test execution receipt or qualification authority.

## Current commands

`just assurance-check` checks catalog shape, mandatory profile command sequences,
unique ownership, live Cargo/Python target discovery, source areas and authority
reference tokens and exact generated CI wiring. It does not execute profiles,
establish reference semantics or grant amendment authority.
`just assurance-workflows` explicitly regenerates workflows; checking never
repairs them. Existing job names and platform coverage remain intact. Hosted
Linux partitions the same mandatory command set across jobs; wall-clock order
differs from the sequential local merge profile.

`just assurance-run PROFILE /absolute/new/output` executes the full registered
profile, retaining per-command logs and `receipt.json`. Output must be new and
outside the repository. It stops on the first mandatory failure, records missing
infrastructure separately, and leaves subsequent commands unexecuted. Interrupted
receipts remain incomplete. Beta is informational; its failure is still recorded.
`just assurance-report /absolute/output/receipt.json` checks receipt consistency
and displays it without rerunning anything. Receipts are not signed attestations.

Cache state defaults to unknown; direct CLI `--cache-state cold|warm|unknown`
records caller classification, not an inferred measurement. Each receipt is one
invocation/sample. Runtime, exits and signals are observed; unavailable peak
memory is null. Resource invocations retain the existing producer's detailed
service limits and peak measurement. Impact is advisory and never skips checks.
Run expensive profiles under explicit host resource limits. Resource controls
enforce their own existing 2-CPU/4-GiB/no-swap/256-task/600-second service bounds.

`just ci` executes the complete Linux merge profile and retains receipts below
`$XDG_STATE_HOME/liminal/assurance` (default `~/.local/state/liminal/assurance`).
Unset `CARGO_TARGET_DIR` for merge: existing crash and sanitizer replay harnesses
require separate checkout-local target directories. Incompatible overrides refuse
before test execution. Hosted parity still needs actual hosted evidence. Calling the
qualification profile invokes the existing HAQP producer, but this runner never
infers qualification from a subprocess exit. Frozen HAQP gates remain authority.

The catalog names targets, not individual assertions. Tests added inside a target
inherit its family. A new target needs explicit classification. Empty/reserved
library targets are listed to detect future growth, not claimed as tested
functionality. Family owners are responsibilities, not human authority enrollment.

## Change checklist

1. Identify changed interface, threat, requirement or dependency. Review affected
   families at each phase gate, and at their declared review triggers.
2. Keep existing assertions. Register new targets and cite existing authorities;
   do not manufacture new authority by editing catalog metadata.
3. Run full merge checks. Development selection cannot substitute for them.
   Preserve both nextest process isolation and cargo's threaded test execution.
4. Keep macOS/Windows coverage. Weekly beta compatibility stays informational;
   advisories remain required for merge as well as scheduled checking.
5. Record actual failures, unavailable infrastructure and unexecuted commands.
   Observe sample counts/environment/cache state before proposing cost budgets.
6. Never automatically retire or quarantine a test, retry a failure, widen a
   timeout or weaken an assertion to reduce maintenance costs.

Coordinate maintenance remains governed by AM-17.13. Planned automatic apply
requires exact unchanged enclosing code, a unique target, independent review and
human-signed policy with external trust pins. Agents never sign or enroll policy.
No automatic apply is implemented or activated by this catalog slice.
