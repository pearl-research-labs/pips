# Pearl Improvement Proposals (PIPs)

This repository tracks **Pearl Improvement Proposals** (PIPs): design documents
that specify new features, processes, or conventions for the Pearl network.

Having a PIP published here indicates that the proposal is in scope and has
met the formal criteria described in PIP-1. It does not indicate that the
proposal has community consensus or that it is about to be adopted. PIP
editors are expected to be liberal with publishing PIPs and to try not to be
too involved in decision-making on behalf of the community.

For the human-friendly reading experience, see the PIPs site (rendered from
this repository): [pips.pearl.dev](https://pips.pearl.dev).

## What is a PIP?

A PIP is a short, focused document that describes a single change or topic,
explains the motivation for it, specifies it precisely enough to implement,
and discusses backwards compatibility, security, and privacy considerations
where relevant.

PIPs are the primary mechanism for proposing changes to the Pearl network,
collecting community input, and recording the design decisions that have
gone into Pearl.

The process and document format are defined in **[PIP-1](./pip-0001.md)**.

## Index

| # | Title | Status | Type | Category |
| - | ----- | ------ | ---- | -------- |
| [1](./pip-0001.md) | PIP Purpose and Guidelines | Active | Process | -- |
| [2](./pip-0002.md) | Grouped-GEMM Proof-of-Useful-Work for Mixture-of-Experts | Draft | Standards Track | Consensus |

## Submitting a new PIP

1. **Discuss first.** Float the idea on the Pearl research forum or in a
   repository issue before writing a PIP, to surface duplicates and gather
   early objections cheaply.
2. **Fork this repository** and copy [`pip-template.md`](./pip-template.md) to
   a new file named `pip-XXXX.md`. Use `XXXX = 9999` while the PIP is in
   flight; the editors will assign a real number when the PIP is merged.
3. **Write the PIP** using the required preamble (see `pip-template.md`) and at
   minimum the sections: Abstract, Motivation, Specification, Rationale,
   Backwards Compatibility (if applicable), Security Considerations
   (if applicable), and Copyright.
4. **Open a pull request** to the `main` branch. The PR description should
   include a one-paragraph summary suitable for someone skimming the change.
5. **Address review.** PIP editors check format, scope, and clarity; the
   community reviews technical content. Update the PIP itself in response to
   review comments rather than arguing in PR threads.
6. **Merge as Draft.** When the format is clean and the scope is well-defined,
   editors assign a PIP number and merge the PR with status `Draft`.
7. **Promote.** Once feature-complete, open a follow-up PR moving the status
   to `Proposed`, then `Final` / `Active` once accepted.

PIP editors are not gatekeepers of merit. They are responsible for ensuring
that PIPs are well-formatted, in scope, and clearly written.

## Statuses

- **Draft**: Under active editing. Merged but not final.
- **Proposed**: Feature-complete, open for broader community review.
- **Active**: Ongoing process or guideline (Process / Informational PIPs).
- **Final**: Accepted Standards Track change that has been deployed.
- **Withdrawn**: Withdrawn by its author(s).
- **Rejected**: Formally rejected; will not be pursued in current form.
- **Replaced**: Obsoleted by a later PIP (see `superseded-by`).
- **Obsolete**: No longer in use; preserved for historical record.

## Copyright

All PIPs in this repository, unless explicitly stated otherwise, are released
into the public domain under
[CC0](https://creativecommons.org/publicdomain/zero/1.0/).
