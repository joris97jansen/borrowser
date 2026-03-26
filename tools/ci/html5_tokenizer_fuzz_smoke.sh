#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORPUS_DIR="${ROOT_DIR}/fuzz/corpus/html5_tokenizer"
BIN_DIR="${ROOT_DIR}/fuzz/target/debug"
BIN_PATH="${BIN_DIR}/html5_tokenizer"
ARTIFACT_DIR="${ROOT_DIR}/ci_artifacts"
ARTIFACT_PATH="${ARTIFACT_DIR}/html5_tokenizer_fuzz_failure"

SEED="${HTML5_TOKENIZER_FUZZ_SMOKE_SEED:-1592653589}"
RUNS="${HTML5_TOKENIZER_FUZZ_SMOKE_RUNS:-128}"
INPUT_TIMEOUT_SEC="${HTML5_TOKENIZER_FUZZ_SMOKE_INPUT_TIMEOUT_SEC:-5}"
WALL_TIMEOUT_SEC="${HTML5_TOKENIZER_FUZZ_SMOKE_WALL_TIMEOUT_SEC:-90}"

mkdir -p "${ARTIFACT_DIR}"
rm -f "${ARTIFACT_PATH}"

echo "html5 tokenizer fuzz smoke"
echo "  corpus: ${CORPUS_DIR}"
echo "  seed: ${SEED}"
echo "  runs: ${RUNS}"
echo "  per-input-timeout-sec: ${INPUT_TIMEOUT_SEC}"
echo "  wall-timeout-sec: ${WALL_TIMEOUT_SEC}"
echo "  failure-artifact: ${ARTIFACT_PATH}"

echo "Building fuzz smoke target..."
cargo build --manifest-path "${ROOT_DIR}/fuzz/Cargo.toml" --bin html5_tokenizer

SMOKE_CMD=(
  "${BIN_PATH}"
  "${CORPUS_DIR}"
  "-seed=${SEED}"
  "-runs=${RUNS}"
  "-timeout=${INPUT_TIMEOUT_SEC}"
  "-exact_artifact_path=${ARTIFACT_PATH}"
)

TIMEOUT_BIN=""
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT_BIN="timeout"
elif command -v gtimeout >/dev/null 2>&1; then
  TIMEOUT_BIN="gtimeout"
fi

echo "Running deterministic fuzz smoke command:"
printf '  %q' "${SMOKE_CMD[@]}"
printf '\n'

run_failure=0
if [[ -n "${TIMEOUT_BIN}" ]]; then
  if "${TIMEOUT_BIN}" "${WALL_TIMEOUT_SEC}" "${SMOKE_CMD[@]}"; then
    run_failure=0
  else
    run_failure=$?
  fi
else
  echo "  note: no timeout/gtimeout found; running without outer wall-timeout wrapper"
  if "${SMOKE_CMD[@]}"; then
    run_failure=0
  else
    run_failure=$?
  fi
fi

if [[ "${run_failure}" -ne 0 ]]; then
  if [[ -f "${ARTIFACT_PATH}" ]]; then
    DIRECT_REPRO_CMD=("${BIN_PATH}" "${ARTIFACT_PATH}")
    CARGO_FUZZ_REPRO_CMD=("cargo" "fuzz" "run" "html5_tokenizer" "${ARTIFACT_PATH}")
  else
    DIRECT_REPRO_CMD=("${SMOKE_CMD[@]}")
    CARGO_FUZZ_REPRO_CMD=("cargo" "fuzz" "run" "html5_tokenizer" "${CORPUS_DIR}")
  fi
  echo
  echo "html5 tokenizer fuzz smoke failed"
  echo "  seed: ${SEED}"
  echo "  corpus: ${CORPUS_DIR}"
  if [[ -f "${ARTIFACT_PATH}" ]]; then
    echo "  failing-input: ${ARTIFACT_PATH}"
  else
    echo "  failing-input: <not materialized>"
  fi
  echo "  direct-repro:"
  printf '    %q' "${DIRECT_REPRO_CMD[@]}"
  printf '\n'
  echo "  cargo-fuzz-repro:"
  printf '    %q' "${CARGO_FUZZ_REPRO_CMD[@]}"
  printf '\n'
  if [[ "${run_failure}" -eq 124 ]]; then
    echo "  failure-kind: wall-timeout"
  else
    echo "  failure-kind: exit-code-${run_failure}"
  fi
  exit "${run_failure}"
fi

echo "html5 tokenizer fuzz smoke passed"
