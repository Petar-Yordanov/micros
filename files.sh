#!/usr/bin/env bash
set -euo pipefail

# Dump file contexts given a series/list of files (relative or absolute).
#
# Examples:
#   ./dump_ctx.sh src/main.rs Makefile
#   ./dump_ctx.sh . src/main.rs Makefile
#   ./dump_ctx.sh --root . src/main.rs Makefile
#   printf '%s\n' src/main.rs Makefile | ./dump_ctx.sh --root . --stdin
#
# Output:
#   - prints a header per file
#   - prints file contents (with line numbers)
#   - continues even if some files are missing (prints MISSING)

ROOT="."
USE_STDIN=0

usage() {
  cat <<'EOF'
Usage:
  dump_ctx.sh [--root ROOT] [--stdin] [--] [FILE...]
  dump_ctx.sh ROOT [FILE...]              (compat: first arg is root if it's a dir and more args follow)

Options:
  --root ROOT   Base directory for relative paths (default: .)
  --stdin       Read file paths (one per line) from stdin. Extra CLI args are also accepted.
  -h, --help    Show help

Notes:
  - Absolute paths are used as-is.
  - Relative paths are resolved as: ROOT/relpath
  - Missing files do not fail the script.
EOF
}

# Parse args
ARGS=()
while (($#)); do
  case "$1" in
    --root)
      shift
      [[ $# -gt 0 ]] || { echo "error: --root requires an argument" >&2; exit 2; }
      ROOT="$1"
      shift
      ;;
    --stdin)
      USE_STDIN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      while (($#)); do ARGS+=("$1"); shift; done
      ;;
    *)
      ARGS+=("$1")
      shift
      ;;
  esac
done

# Back-compat: if first positional looks like a directory AND there are more args,
# treat it as ROOT (like your old script did with $1 defaulting to '.').
if [[ ${#ARGS[@]} -ge 2 && -d "${ARGS[0]}" ]]; then
  ROOT="${ARGS[0]}"
  ARGS=("${ARGS[@]:1}")
fi

# Collect files: from stdin (optional) + from args
FILES=()

if [[ $USE_STDIN -eq 1 ]]; then
  # Read non-empty lines; allow spaces in paths.
  while IFS= read -r line; do
    [[ -n "${line//[[:space:]]/}" ]] || continue
    FILES+=("$line")
  done
fi

if [[ ${#ARGS[@]} -gt 0 ]]; then
  FILES+=("${ARGS[@]}")
fi

if [[ ${#FILES[@]} -eq 0 ]]; then
  echo "error: no files provided (pass FILE... or use --stdin)" >&2
  usage >&2
  exit 2
fi

print_file() {
  local input="$1"
  local path

  if [[ "$input" = /* ]]; then
    path="$input"
  else
    path="$ROOT/$input"
  fi

  echo
  echo "===== BEGIN FILE: $input ====="
  if [[ -f "$path" ]]; then
    # Print line numbers for easier patch guidance
    nl -ba "$path"
  else
    echo "MISSING: $input"
  fi
  echo "===== END FILE: $input ====="
}

echo "CTX_DUMP_ROOT=$ROOT"
echo "CTX_DUMP_DATE_UTC=$(date -u +'%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || true)"

for f in "${FILES[@]}"; do
  print_file "$f"
done
