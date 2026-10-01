#!/usr/bin/env bash
set -euo pipefail

VERSION="${1:-}"

if [[ -z "${VERSION}" ]]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

if [[ "${DRY_RUN:-false}" == "true" ]]; then
  echo "==> Skip CHANGELOG update in dry-run"
  exit 0
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "${ROOT_DIR}"

echo "==> Update CHANGELOG.md for v${VERSION}"

set +e
python3 - "${VERSION}" <<'PY'
import datetime
import re
import sys
from pathlib import Path

version = sys.argv[1]
path = Path("CHANGELOG.md")
text = path.read_text()

if re.search(rf"^## \[{re.escape(version)}\]", text, re.M):
    print(f"CHANGELOG already has [{version}], leaving as-is")
    sys.exit(0)

m = re.search(r"^## \[Unreleased\]\s*\n", text, re.M)
if not m:
    print("No [Unreleased] section; falling back to git-cliff", file=sys.stderr)
    sys.exit(2)

rest = text[m.end() :]
next_h = re.search(r"^## \[", rest, re.M)
body = rest[: next_h.start()] if next_h else rest
if not body.strip():
    print("[Unreleased] empty; falling back to git-cliff", file=sys.stderr)
    sys.exit(2)

date = datetime.date.today().isoformat()
new_header = f"## [Unreleased]\n\n## [{version}] - {date}\n"
path.write_text(text[: m.start()] + new_header + rest)
print(f"Promoted [Unreleased] → [{version}] - {date}")
PY
status=$?
set -e

if [[ "${status}" -eq 2 ]]; then
  git cliff --config cliff.toml --tag "v${VERSION}" --output CHANGELOG.md
elif [[ "${status}" -ne 0 ]]; then
  exit "${status}"
fi
