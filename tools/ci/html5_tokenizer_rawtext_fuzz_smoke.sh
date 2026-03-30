#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CORPUS_DIR="${ROOT_DIR}/fuzz/corpus/html5_tokenizer_rawtext"
REGRESSION_DIR="${ROOT_DIR}/fuzz/regressions/html5_tokenizer_rawtext"
BIN_DIR="${ROOT_DIR}/fuzz/target/debug"
BIN_PATH="${BIN_DIR}/html5_tokenizer_rawtext"
ARTIFACT_DIR="${ROOT_DIR}/ci_artifacts"
ARTIFACT_BASENAME="${HTML5_TOKENIZER_RAWTEXT_FUZZ_ARTIFACT_BASENAME:-html5_tokenizer_rawtext_fuzz_failure}"
ARTIFACT_PATH="${ARTIFACT_DIR}/${ARTIFACT_BASENAME}"
LABEL="${HTML5_TOKENIZER_RAWTEXT_FUZZ_LABEL:-html5 tokenizer rawtext fuzz smoke}"

SEED="${HTML5_TOKENIZER_RAWTEXT_FUZZ_SMOKE_SEED:-2654435761}"
RUNS="${HTML5_TOKENIZER_RAWTEXT_FUZZ_SMOKE_RUNS:-128}"
INPUT_TIMEOUT_SEC="${HTML5_TOKENIZER_RAWTEXT_FUZZ_SMOKE_INPUT_TIMEOUT_SEC:-5}"
WALL_TIMEOUT_SEC="${HTML5_TOKENIZER_RAWTEXT_FUZZ_SMOKE_WALL_TIMEOUT_SEC:-90}"

mkdir -p "${ARTIFACT_DIR}"
rm -f "${ARTIFACT_PATH}"

INPUT_DIRS=("${CORPUS_DIR}")
if [[ -d "${REGRESSION_DIR}" ]] && find "${REGRESSION_DIR}" -maxdepth 1 -type f ! -name '.*' ! -name '*.md' | grep -q .; then
  INPUT_DIRS+=("${REGRESSION_DIR}")
fi

echo "${LABEL}"
echo "  corpus: ${CORPUS_DIR}"
echo "  regressions: ${REGRESSION_DIR}"
echo "  input-dirs: ${INPUT_DIRS[*]}"
echo "  seed: ${SEED}"
echo "  runs: ${RUNS}"
echo "  per-input-timeout-sec: ${INPUT_TIMEOUT_SEC}"
echo "  wall-timeout-sec: ${WALL_TIMEOUT_SEC}"
echo "  failure-artifact: ${ARTIFACT_PATH}"

echo "Building fuzz smoke target..."
cargo build --manifest-path "${ROOT_DIR}/fuzz/Cargo.toml" --bin html5_tokenizer_rawtext

SMOKE_CMD=(
  "${BIN_PATH}"
  "${INPUT_DIRS[@]}"
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

echo "Running deterministic fuzz command:"
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
    CARGO_FUZZ_REPRO_CMD=("cargo" "fuzz" "run" "html5_tokenizer_rawtext" "${ARTIFACT_PATH}")
  else
    DIRECT_REPRO_CMD=("${SMOKE_CMD[@]}")
    CARGO_FUZZ_REPRO_CMD=("cargo" "fuzz" "run" "html5_tokenizer_rawtext" "${INPUT_DIRS[@]}")
  fi
  echo
  echo "${LABEL} failed"
  echo "  seed: ${SEED}"
  echo "  corpus: ${CORPUS_DIR}"
  echo "  regressions: ${REGRESSION_DIR}"
  echo "  input-dirs: ${INPUT_DIRS[*]}"
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
  echo "  triage-store:"
  echo "    ${REGRESSION_DIR}/<descriptive-name>"
  if [[ "${run_failure}" -eq 124 ]]; then
    echo "  failure-kind: wall-timeout"
  else
    echo "  failure-kind: exit-code-${run_failure}"
  fi
  exit "${run_failure}"
fi

echo "${LABEL} passed"
