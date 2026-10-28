# Review protocol

## Mechanical checks

Run commands from the repository root and record exit codes.

| Mode | Required checks |
| --- | --- |
| `quick` | Targeted tests for the changed area; use `npm run quality:static` when TypeScript, Svelte, shared UI logic, or build configuration changed. |
| `ui` | `npm run quality:static`; render the affected surface when feasible; run targeted interaction tests. |
| `release` | `npm run quality:release`; inspect version alignment and release configuration; run `npm run qa:windows` only when `release/current/qingjian.exe` exists and is the intended current build. |

If `qa:windows` cannot run, label it `SKIPPED` with the missing prerequisite. A skipped lifecycle test prevents an unconditional release pass when the change touches startup, shutdown, tray, single-instance, updater, or window restoration behavior.

## Independent reviewer briefs

Every reviewer is read-only and receives the scope, relevant diff or baseline goal, product constraints, and required output schema. Do not give a reviewer prior conclusions.

- Fresh-eyes engineering: look for concrete regressions, incorrect assumptions, missing failure handling, data loss, lifecycle breakage, compatibility risk, weak tests, and unjustified complexity.
- Product/UX: inspect task success, discoverability, hierarchy, keyboard and focus behavior, feedback, recovery, state coverage, microcopy, consistency, accessibility, and Windows conventions.
- Release/data safety: inspect version consistency, updater/signing configuration, migrations/backups, local-only guarantees, build/release gates, and operational rollback.

Reviewers must not edit files. Their output must cite a file and line, rendered state, or command result for every finding.

## Finding schema

Use one block per finding:

- `ID`: stable prefix such as `ENG-01`, `UX-01`, or `REL-01`.
- `Severity`: `Blocker`, `High`, `Medium`, or `Low` based on user consequence.
- `Confidence`: percentage plus `Observed`, `Inferred`, or `Hypothesis` and the evidence source.
- `Location`: file and line, screen/flow, or command.
- `Evidence`: the exact behavior or output observed.
- `Impact`: what the user or release loses.
- `Recommendation`: the smallest concrete correction.
- `Effort`: `S`, `M`, or `L`.

Do not turn taste into a defect. Label a subjective design preference as `Taste` and never grade it Blocker or High.

## Report layout

Write `.reviews/<YYYY-MM-DD-HHmm>-<mode>/summary.md` containing:

1. Scope and evidence base.
2. Mechanical check table with commands, exit codes, and skipped prerequisites.
3. Independent reviewer coverage and disagreements.
4. Findings ordered by severity.
5. What is working and intentionally left unchanged.
6. Decision: `PASS`, `PASS WITH NOTES`, or `BLOCKED`.
7. Required approvals and verification steps.

Keep raw outputs in sibling text files when they are too long for the summary. `.reviews/` is temporary and ignored by Git.

For a formal release only, distill the approved final report to `docs/reviews/releases/v<version>.md`. Include the source commit, release version, completed checks, explicit skips, resolved blockers, remaining accepted risks, and final decision. Do not copy transient reviewer chatter or raw logs into the tracked archive.
