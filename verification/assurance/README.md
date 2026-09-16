# Maintaining assurance

Authority: [ADR-0022](../../docs/adr/0022-maintain-assurance-without-requalifying.md).
Delivery status: [PLAN.md](PLAN.md). This catalog is classification metadata, not
a coverage proof, test execution receipt or qualification authority.

## Current command

`just assurance-check` checks catalog shape, mandatory profile command sequences,
unique ownership, live Cargo/Python target discovery, source areas and authority
reference tokens. It does not execute profiles, validate generated CI wiring,
establish reference semantics or grant amendment authority. Those later delivery
steps remain unchecked in the plan.

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
