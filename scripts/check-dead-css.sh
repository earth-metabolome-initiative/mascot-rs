#!/usr/bin/env bash
#
# Fails if any CSS class defined in the mascot-web stylesheet is never
# referenced in the Rust sources. Class names in the app are always written as
# verbatim string literals in rsx `class:` attributes (plain or in if/else
# branches), so a textual check is exact: every used class appears as a
# space/quote-delimited token somewhere under src/.
#
# Usage: bash scripts/check-dead-css.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CSS="$ROOT/crates/mascot-web/assets/main.css"
SRC="$ROOT/crates/mascot-web/src"

if [ ! -f "$CSS" ]; then
    echo "stylesheet not found: $CSS" >&2
    exit 1
fi

# Class names defined in the stylesheet: take the selector text before each '{'
# (one rule per line), then pull out every `.class` token.
classes="$(
    grep -oE '^[^{]*\{' "$CSS" \
        | sed 's/{.*//' \
        | grep -oE '\.[A-Za-z_][A-Za-z0-9_-]*' \
        | sed 's/^\.//' \
        | sort -u
)"

dead=()
for class in $classes; do
    # Match the class as a whole token inside a class string: bounded by a quote
    # or whitespace on both sides (avoids the `segment` vs `segmented` trap).
    pattern="[\"[:space:]]${class}[\"[:space:]]"
    if ! grep -rqE --include='*.rs' "$pattern" "$SRC"; then
        dead+=("$class")
    fi
done

if [ "${#dead[@]}" -gt 0 ]; then
    echo "Dead CSS classes (defined in main.css, not referenced in src/):" >&2
    printf '  .%s\n' "${dead[@]}" >&2
    exit 1
fi

echo "check-dead-css: no dead CSS classes."
