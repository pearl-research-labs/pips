#!/usr/bin/env python3
"""Validate the front matter of every PIP under PIPS/.

The YAML preamble in each ``PIPS/pip-XXXX.md`` file is the machine-readable
source of truth for the index rendered on the PIPs website. CI runs this
script on every push and pull request to keep preambles consistent with the
vocabulary and format rules defined in PIP-1.
"""

from __future__ import annotations

import re
import sys
from datetime import date
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PIP_DIR = REPO_ROOT / "PIPS"
PIP_GLOB = "pip-[0-9]*.md"

FENCE = "---"

# Allowed values, kept in sync with the PIP-1 process document.
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
ALLOWED_CATEGORIES = frozenset({"Consensus", "Networking", "Interface", "Applications"})

REQUIRED_FIELDS = ("pip", "title", "description", "author", "status", "type", "created")

# PIP-1 asks for titles of roughly 60 characters; enforce a hard ceiling.
MAX_TITLE_LENGTH = 72

CREATED_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


class PipError(Exception):
    """Raised when a PIP file has invalid or missing front-matter."""


def _unquote(value: str) -> str:
    """Strip a single pair of matching surrounding quotes, if present."""
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {'"', "'"}:
        return value[1:-1]
    return value


def parse_front_matter(path: Path) -> dict[str, str]:
    """Return the flat key/value front-matter of a PIP markdown file."""
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0].strip() != FENCE:
        raise PipError(f"{path.name}: missing front-matter (no leading '---')")

    fields: dict[str, str] = {}
    closed = False
    for raw_line in lines[1:]:
        if raw_line.strip() == FENCE:
            closed = True
            break
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            raise PipError(f"{path.name}: malformed front-matter line: {raw_line!r}")
        key, value = line.split(":", 1)
        fields[key.strip()] = _unquote(value.strip())

    if not closed:
        raise PipError(f"{path.name}: unterminated front-matter block")
    return fields


def _parse_pip_refs(value: str) -> list[int]:
    """Parse a comma-separated list of PIP numbers (e.g. ``1`` or ``1, 2``)."""
    return [int(part.strip()) for part in value.split(",")]


def validate_pip(path: Path) -> tuple[int | None, list[str]]:
    """Return ``(pip_number, error_messages)`` for a single PIP file."""
    errors: list[str] = []

    try:
        fields = parse_front_matter(path)
    except PipError as exc:
        return None, [str(exc)]

    missing = [field for field in REQUIRED_FIELDS if not fields.get(field)]
    if missing:
        errors.append(f"{path.name}: missing required field(s): {', '.join(missing)}")

    number: int | None = None
    if fields.get("pip"):
        try:
            number = int(fields["pip"])
        except ValueError:
            errors.append(f"{path.name}: 'pip' must be an integer, got {fields['pip']!r}")
        else:
            expected_name = f"pip-{number:04d}.md"
            if path.name != expected_name:
                errors.append(
                    f"{path.name}: filename does not match pip number "
                    f"({expected_name} expected)"
                )

    title = fields.get("title", "")
    if len(title) > MAX_TITLE_LENGTH:
        errors.append(
            f"{path.name}: title is {len(title)} characters; "
            f"PIP-1 asks for ~60 (hard limit {MAX_TITLE_LENGTH})"
        )

    status = fields.get("status", "")
    if status and status not in ALLOWED_STATUSES:
        errors.append(f"{path.name}: unknown status {status!r}")

    pip_type = fields.get("type", "")
    if pip_type and pip_type not in ALLOWED_TYPES:
        errors.append(f"{path.name}: unknown type {pip_type!r}")

    category = fields.get("category")
    if pip_type == "Standards Track":
        if not category:
            errors.append(f"{path.name}: Standards Track PIPs require a 'category'")
        elif category not in ALLOWED_CATEGORIES:
            errors.append(f"{path.name}: unknown category {category!r}")
    elif category:
        errors.append(
            f"{path.name}: 'category' is only allowed for Standards Track PIPs"
        )

    created = fields.get("created", "")
    if created:
        if not CREATED_RE.match(created):
            errors.append(
                f"{path.name}: 'created' must be an ISO date (YYYY-MM-DD), "
                f"got {created!r}"
            )
        else:
            try:
                date.fromisoformat(created)
            except ValueError:
                errors.append(f"{path.name}: 'created' is not a valid date: {created!r}")

    if status == "Replaced" and not fields.get("superseded-by"):
        errors.append(
            f"{path.name}: status 'Replaced' requires a 'superseded-by' field"
        )

    for key in ("requires", "replaces", "superseded-by"):
        value = fields.get(key)
        if value:
            try:
                _parse_pip_refs(value)
            except ValueError:
                errors.append(
                    f"{path.name}: '{key}' must be a comma-separated list of "
                    f"PIP numbers, got {value!r}"
                )

    discussions = fields.get("discussions-to")
    if discussions and not discussions.startswith(("http://", "https://")):
        errors.append(f"{path.name}: 'discussions-to' must be a URL, got {discussions!r}")

    return number, errors


def main() -> int:
    paths = sorted(PIP_DIR.glob(PIP_GLOB))
    if not paths:
        print(f"error: no PIP files found in {PIP_DIR}", file=sys.stderr)
        return 2

    all_errors: list[str] = []
    seen: dict[int, str] = {}
    for path in paths:
        number, errors = validate_pip(path)
        all_errors.extend(errors)
        if number is not None:
            if number in seen:
                all_errors.append(
                    f"duplicate pip number {number}: {seen[number]} and {path.name}"
                )
            else:
                seen[number] = path.name

    if all_errors:
        for error in all_errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    print(f"{len(paths)} PIP file(s) validated, no problems found.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
