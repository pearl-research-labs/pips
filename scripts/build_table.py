#!/usr/bin/env python3
"""Generate and validate the PIP Index table in README.md.

The PIP front-matter (the YAML preamble in each ``pip-XXXX.md`` file) is the
source of truth. This script rebuilds the Index table from those preambles and
writes it into README.md between the index markers. In ``--check`` mode it
fails (exit code 1) if the committed table is out of sync, which lets CI catch
a README that was not regenerated after a PIP changed.
"""

from __future__ import annotations

import argparse
import difflib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
README_PATH = REPO_ROOT / "README.md"
PIP_GLOB = "pip-[0-9]*.md"

BEGIN_MARKER = "<!-- BEGIN PIP INDEX"
END_MARKER = "<!-- END PIP INDEX -->"

EMPTY_FIELD = "--"
TABLE_HEADER = "| # | Title | Status | Type | Category |"
TABLE_DIVIDER = "| - | ----- | ------ | ---- | -------- |"

# Allowed values, kept in sync with the "Statuses" section of README.md and the
# PIP-1 process document. Used to validate front-matter before building rows.
ALLOWED_STATUSES = frozenset(
    {
        "Draft",
        "Proposed",
        "Active",
        "Final",
        "Withdrawn",
        "Rejected",
        "Replaced",
        "Obsolete",
    }
)
ALLOWED_TYPES = frozenset({"Standards Track", "Process", "Informational"})

REQUIRED_FIELDS = ("pip", "title", "status", "type")


class PipError(Exception):
    """Raised when a PIP file has invalid or missing front-matter."""


def parse_front_matter(path: Path) -> dict[str, str]:
    """Return the flat key/value front-matter of a PIP markdown file."""
    text = path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        raise PipError(f"{path.name}: missing front-matter (no leading '---')")

    # Split on the first two '---' fences.
    parts = text.split("---", 2)
    if len(parts) < 3:
        raise PipError(f"{path.name}: unterminated front-matter block")

    fields: dict[str, str] = {}
    for raw_line in parts[1].splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            raise PipError(f"{path.name}: malformed front-matter line: {raw_line!r}")
        key, value = line.split(":", 1)
        fields[key.strip()] = value.strip()
    return fields


def build_row(path: Path) -> tuple[int, str]:
    """Return ``(pip_number, markdown_row)`` for a single PIP file."""
    fields = parse_front_matter(path)

    missing = [field for field in REQUIRED_FIELDS if not fields.get(field)]
    if missing:
        raise PipError(f"{path.name}: missing required field(s): {', '.join(missing)}")

    try:
        number = int(fields["pip"])
    except ValueError as exc:
        raise PipError(f"{path.name}: 'pip' must be an integer, got {fields['pip']!r}") from exc

    expected_name = f"pip-{number:04d}.md"
    if path.name != expected_name:
        raise PipError(f"{path.name}: filename does not match pip number ({expected_name} expected)")

    status = fields["status"]
    if status not in ALLOWED_STATUSES:
        raise PipError(f"{path.name}: unknown status {status!r}")

    pip_type = fields["type"]
    if pip_type not in ALLOWED_TYPES:
        raise PipError(f"{path.name}: unknown type {pip_type!r}")

    category = fields.get("category") or EMPTY_FIELD
    row = (
        f"| [{number}](./{path.name}) | {fields['title']} | "
        f"{status} | {pip_type} | {category} |"
    )
    return number, row


def build_table() -> str:
    """Build the full Index table (header + divider + rows) from PIP files."""
    rows: list[tuple[int, str]] = []
    seen: dict[int, str] = {}
    for path in sorted(REPO_ROOT.glob(PIP_GLOB)):
        number, row = build_row(path)
        if number in seen:
            raise PipError(f"duplicate pip number {number}: {seen[number]} and {path.name}")
        seen[number] = path.name
        rows.append((number, row))

    rows.sort(key=lambda item: item[0])
    lines = [TABLE_HEADER, TABLE_DIVIDER, *(row for _, row in rows)]
    return "\n".join(lines)


def replace_between_markers(readme: str, table: str) -> str:
    """Return README text with the index region replaced by ``table``."""
    pattern = re.compile(
        re.escape(BEGIN_MARKER) + r".*?-->\n.*?\n" + re.escape(END_MARKER),
        re.DOTALL,
    )
    match = pattern.search(readme)
    if match is None:
        raise PipError(
            f"could not find index markers ({BEGIN_MARKER} ... {END_MARKER}) in README.md"
        )
    begin_line = match.group(0).split("\n", 1)[0]
    replacement = f"{begin_line}\n{table}\n{END_MARKER}"
    return readme[: match.start()] + replacement + readme[match.end() :]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit non-zero if README.md is out of sync instead of rewriting it",
    )
    args = parser.parse_args()

    try:
        table = build_table()
        readme = README_PATH.read_text(encoding="utf-8")
        updated = replace_between_markers(readme, table)
    except PipError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2

    if args.check:
        if updated != readme:
            diff = difflib.unified_diff(
                readme.splitlines(keepends=True),
                updated.splitlines(keepends=True),
                fromfile="README.md (committed)",
                tofile="README.md (expected)",
            )
            sys.stderr.writelines(diff)
            print(
                "\nerror: README.md Index table is out of date. "
                "Run `python scripts/build_table.py` and commit the result.",
                file=sys.stderr,
            )
            return 1
        print("README.md Index table is up to date.")
        return 0

    README_PATH.write_text(updated, encoding="utf-8")
    print("README.md Index table regenerated.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
