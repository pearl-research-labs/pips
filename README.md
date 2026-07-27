# Pearl Improvement Proposals (PIPs)

Pearl Improvement Proposals (PIPs) are design documents that specify new
features, processes, or conventions for the Pearl network. Each PIP describes
a single change, explains its motivation, specifies it precisely enough to
implement, and discusses backwards compatibility, security, and privacy
considerations where relevant.

The canonical, rendered index of all proposals lives at
**[pearl-research-labs.github.io/pips](https://pearl-research-labs.github.io/pips/)**.
The proposal sources live in [`PIPS/`](./PIPS/), with per-PIP images and
diagrams under [`assets/`](./assets/).

The process and document format are defined in
[PIP-1: PIP Purpose and Guidelines](./PIPS/pip-0001.md).

## Submitting a new PIP

1. **Discuss first.** Float the idea on the Pearl research forum or in a
   [repository issue](https://github.com/pearl-research-labs/pips/issues) to
   surface duplicates and gather early objections cheaply.
2. **Fork this repository** and copy [`pip-template.md`](./pip-template.md) to
   `PIPS/pip-9999.md`; the editors assign the real number when the PIP is
   merged.
3. **Write the PIP** following the preamble and section requirements of
   [PIP-1](./PIPS/pip-0001.md).
4. **Open a pull request.** See [CONTRIBUTING.md](./CONTRIBUTING.md) for the
   checks CI runs against every PR and how to run them locally.

Having a PIP published here indicates that the proposal is in scope and has
met the formal criteria described in PIP-1. It does not indicate that the
proposal has community consensus or that it is about to be adopted. PIP
editors check format, scope, and clarity; the community judges technical
merit.

## Statuses

A PIP enters as `Draft`, moves to `Proposed` when feature-complete, and ends
as `Final` (deployed Standards Track changes) or `Active` (ongoing Process and
Informational documents). `Withdrawn`, `Rejected`, `Replaced`, and `Obsolete`
mark proposals that are no longer pursued. PIP-1 defines the
[full lifecycle](./PIPS/pip-0001.md#pip-statuses).

## Copyright

All PIPs in this repository, unless explicitly stated otherwise, are released
into the public domain under
[CC0](https://creativecommons.org/publicdomain/zero/1.0/).
