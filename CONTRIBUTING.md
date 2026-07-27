# Contributing Guidelines

Please read [PIP-1](./PIPS/pip-0001.md) for the full PIP process and format
requirements before submitting a proposal.

## Quick start

1. Discuss the idea on the Pearl research forum or in a repository issue.
2. Fork this repository and copy [`pip-template.md`](./pip-template.md) to
   `PIPS/pip-9999.md`.
3. Write the PIP following the required preamble and section structure. Place
   any images or diagrams in `assets/pip-9999/` and reference them with
   relative links.
4. Open a pull request to the `main` branch.

## Local checks

CI runs these checks on every push and pull request; you can run them locally
before opening a PR.

**Front-matter validation** (requires Python 3.10+):

```bash
python scripts/validate_pips.py
```

**Typo check** (requires [`typos`](https://github.com/crate-ci/typos)):

```bash
typos
```

**Markdown lint** (requires Node.js):

```bash
npx markdownlint-cli2 "**/*.md"
```

**Markdown link check** (requires
[`markdown-link-check`](https://github.com/tcort/markdown-link-check)):

```bash
find . -name '*.md' -not -path './node_modules/*' -exec markdown-link-check --config .markdown-link-check.json {} \;
```

## Previewing the website

The site at
[pearl-research-labs.github.io/pips](https://pearl-research-labs.github.io/pips/)
is built with [Hugo](https://gohugo.io/) and deployed automatically from
`main` by GitHub Actions; PIP authors never need to build it themselves. To
preview changes locally, [install Hugo](https://gohugo.io/installation/) and
run:

```bash
hugo server
```

then open `http://localhost:1313/pips/`.
