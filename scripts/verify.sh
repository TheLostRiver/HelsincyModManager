#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${repo_root}" ]]; then
  echo "Current directory is not inside a Git repository." >&2
  exit 1
fi

policy_only=false
if [[ "${1:-}" == "--policy-only" ]]; then
  policy_only=true
elif [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  echo "Usage: scripts/verify.sh [--policy-only]"
  exit 0
elif [[ $# -gt 0 ]]; then
  echo "Unknown argument: $1" >&2
  echo "Usage: scripts/verify.sh [--policy-only]" >&2
  exit 1
fi

cd "${repo_root}"

resolve_command() {
  local command_name="$1"
  local windows_name="${2:-$1}"

  if command -v "${command_name}" >/dev/null 2>&1; then
    command -v "${command_name}"
    return 0
  fi

  if command -v "${windows_name}" >/dev/null 2>&1; then
    command -v "${windows_name}"
    return 0
  fi

  if command -v where.exe >/dev/null 2>&1; then
    local windows_path
    windows_path="$(where.exe "${windows_name}" 2>/dev/null | tr -d '\r' | head -n 1 || true)"
    if [[ -n "${windows_path}" ]]; then
      if command -v cygpath >/dev/null 2>&1; then
        cygpath -u "${windows_path}"
      else
        echo "${windows_path}"
      fi
      return 0
    fi
  fi

  echo "Required command is missing: ${command_name}" >&2
  return 1
}

resolve_posix_command() {
  local command_name="$1"
  local candidate

  if ! candidate="$(command -v "${command_name}" 2>/dev/null)"; then
    echo "Required command is missing: ${command_name}" >&2
    return 1
  fi

  if [[ -f "${candidate}" ]] && head -n 1 "${candidate}" | grep -q $'\r'; then
    echo "Required command resolves to a Windows shim that Bash cannot execute: ${candidate}" >&2
    echo "Install ${command_name} inside the Linux environment, or use scripts/verify.ps1 on Windows." >&2
    return 1
  fi

  echo "${candidate}"
}

assert_required_file() {
  local relative_path="$1"
  if [[ ! -f "${relative_path}" ]]; then
    echo "Required file is missing: ${relative_path}" >&2
    exit 1
  fi
}

invoke_pnpm() {
  "${corepack_bin}" pnpm "$@"
}

node_bin="$(resolve_command node node.exe)"

echo "Running native policy checks..."
"${node_bin}" scripts/check-policy.mjs --scope verify

if [[ -f "src-tauri/tauri.conf.json" ]]; then
  echo "Checking Tauri icon assets..."
  assert_required_file "src-tauri/icons/icon.ico"
  assert_required_file "src-tauri/icons/icon.png"
fi

if [[ "${policy_only}" == "true" ]]; then
  echo "Policy verification passed."
  exit 0
fi

echo "Running verification entrypoint contract tests..."
"${node_bin}" --test scripts/verify-entrypoints.test.mjs

if [[ -f "package.json" ]]; then
  corepack_bin="$(resolve_posix_command corepack)"

  if [[ ! -d "node_modules" ]]; then
    echo "node_modules is missing. Run: corepack pnpm install --frozen-lockfile" >&2
    exit 1
  fi

  echo "Running frontend typecheck..."
  invoke_pnpm run typecheck

  echo "Running frontend lint..."
  invoke_pnpm run lint

  echo "Running frontend tests..."
  invoke_pnpm run test

  echo "Running frontend build..."
  invoke_pnpm run build
else
  echo "Skipping frontend checks: package.json does not exist yet."
fi

if [[ -f "Cargo.toml" ]]; then
  echo "Running Rust tests..."
  cargo test --workspace

  echo "Running Rust check..."
  cargo check --workspace

  echo "Running Rust clippy..."
  cargo clippy --workspace --all-targets -- -D warnings
else
  echo "Skipping Rust checks: Cargo.toml does not exist yet."
fi

echo "Verification passed."
