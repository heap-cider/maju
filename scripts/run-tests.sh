#!/usr/bin/env bash
# =============================================================================
# run-tests.sh — Run Maju test suite
# =============================================================================
# Usage:
#   ./scripts/run-tests.sh              # run all tests (default)
#   ./scripts/run-tests.sh unit         # unit tests only (no infra needed)
#   ./scripts/run-tests.sh integration  # integration tests only
#   ./scripts/run-tests.sh all          # explicit all
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
MODE="${1:-all}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log()    { echo -e "${BLUE}[run-tests]${NC} $*"; }
success(){ echo -e "${GREEN}[run-tests]${NC} $*"; }
warn()   { echo -e "${YELLOW}[run-tests]${NC} $*"; }
error()  { echo -e "${RED}[run-tests]${NC} $*" >&2; }
section(){ echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; echo -e "${CYAN}  $*${NC}"; echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"; }

cd "${REPO_ROOT}"

# ---- Load .env if present ---------------------------------------------------

if [[ -f ".env" ]]; then
  log "Loading .env..."
  set -o allexport
  # shellcheck disable=SC1091
  source .env
  set +o allexport
else
  # Use defaults matching docker-compose.yml
  export DATABASE_URL="postgres://maju:maju_dev@localhost:5432/maju" # sadscan:disable np.postgres.1
  export PGHOST=localhost
  export PGPORT=5432
  export PGUSER=maju
  export PGPASSWORD=maju_dev
  export PGDATABASE=maju
  export REDIS_URL="redis://localhost:6379"
fi

# ---- Track results ----------------------------------------------------------

declare -a PASSED=()
declare -a FAILED=()

run_test_step() {
  local name="$1"
  shift
  log "Running: ${name}"
  if "$@"; then
    success "${name} passed"
    PASSED+=("${name}")
  else
    error "${name} FAILED"
    FAILED+=("${name}")
  fi
}

# ---- Check / start infra (for integration tests) ----------------------------

ensure_infra() {
  "${REPO_ROOT}/bin/just" _ensure-migrations
}

# ---- Unit tests (no infra needed) -------------------------------------------

run_unit_tests() {
  section "Unit Tests (no infra required)"

  run_test_step "maju-core tests" \
    cargo test -p maju-core --lib -- --nocapture

  run_test_step "maju-auth unit tests" \
    cargo test -p maju-auth --lib -- --nocapture

  run_test_step "maju-voice tests" \
    cargo test -p maju-voice --lib -- --nocapture

  run_test_step "maju-cli tests" \
    cargo test -p maju-cli -- --nocapture

  # maju-db migrator/lint unit tests (no infra): guard the embedded-migrator
  # invariant (exactly the consolidated 0001; cutover/backfill stays an operator
  # script, not startup state) and the tenant-scoping lints. The Postgres-backed
  # maju-db tests are #[ignore]d; nothing here (or in integration mode below,
  # which runs `cargo test -p maju-db` without --ignored) runs them — they need a
  # separate isolated-DB gate, so --lib keeps this step infra-free.
  run_test_step "maju-db unit tests" \
    cargo test -p maju-db --lib -- --nocapture

  # Multi-tenant conformance gate: independent replay checker + golden
  # fixtures (maju-conformance). Pure in-process trace replay, no infra.
  run_test_step "maju-conformance tests" \
    cargo test -p maju-conformance -- --nocapture

  run_test_step "maju-push-gateway tests" \
    cargo test -p maju-push-gateway -- --nocapture
  # maju-agent model-capabilities corpus: the Rust half of the cross-language
  # drift guard. model_capabilities.rs embeds scripts/model-capabilities.json +
  # scripts/normative-corpus.json via include_str! and replays the full locked
  # corpus as pure in-process tests (no infra). Mirrors the nextest path in
  # `just test-unit` — the two lists must stay in step.
  run_test_step "maju-agent unit tests" \
    cargo test -p maju-agent --lib -- --nocapture

  # ACP author-gate and queue tests are pure unit tests. Keep this fallback in
  # step with `just test-unit`; ignored lifecycle tests run elsewhere.
  run_test_step "maju-acp unit tests" \
    cargo test -p maju-acp --lib -- --nocapture
}

# ---- DB / integration tests (infra required) --------------------------------

run_integration_tests() {
  section "Integration Tests (requires running services)"

  ensure_infra

  run_test_step "maju-db tests" \
    cargo test -p maju-db -- --nocapture

  if find crates/maju-auth/tests -maxdepth 1 -name '*.rs' -print -quit 2>/dev/null | grep -q .; then
    run_test_step "maju-auth integration tests" \
      cargo test -p maju-auth --test '*' -- --nocapture
  else
    run_test_step "maju-auth (no integration tests found)" true
  fi

  run_test_step "workspace integration tests" \
    cargo test --test '*' -- --nocapture 2>/dev/null || \
    run_test_step "workspace integration tests (none found)" true
}

# ---- Main -------------------------------------------------------------------

START_TIME=$(date +%s)

case "${MODE}" in
  unit)
    run_unit_tests
    ;;
  integration)
    run_integration_tests
    ;;
  all|*)
    run_unit_tests
    run_integration_tests
    ;;
esac

END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))

# ---- Summary ----------------------------------------------------------------

section "Test Summary"
echo ""
echo -e "  Duration: ${ELAPSED}s"
echo ""

if [[ ${#PASSED[@]} -gt 0 ]]; then
  echo -e "  ${GREEN}Passed (${#PASSED[@]}):${NC}"
  for t in "${PASSED[@]}"; do
    echo -e "    ${GREEN}pass${NC} ${t}"
  done
fi

if [[ ${#FAILED[@]} -gt 0 ]]; then
  echo ""
  echo -e "  ${RED}Failed (${#FAILED[@]}):${NC}"
  for t in "${FAILED[@]}"; do
    echo -e "    ${RED}fail${NC} ${t}"
  done
  echo ""
  exit 1
fi

echo ""
success "All tests passed!"
exit 0
