# Contributing Guidelines

Please read [PIP-1](./pip-0001.md) for the full PIP process and format
requirements before submitting a proposal.

## Quick start

1. Discuss the idea on the Pearl research forum or in a repository issue.
2. Fork this repository and copy [`pip-template.md`](./pip-template.md) to
   `pip-9999.md`.
3. Write the PIP following the required preamble and section structure.
4. Open a pull request to the `main` branch.

## Local checks

This repository uses CI checks on every push and pull request. You can run
them locally before opening a PR.

**Typo check** (requires [`typos`](https://github.com/crate-ci/typos)):

```bash
typos
```

**Markdown link check** (requires
[`markdown-link-check`](https://github.com/tcort/markdown-link-check)):

```bash
find . -name '*.md' -not -path './node_modules/*' -exec markdown-link-check {} \;
```
