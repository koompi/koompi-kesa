#!/usr/bin/env bash
# Run cargo with explicit build/temp storage and fail-fast filesystem headroom
# checks. This prevents long all-target runs from dying late during linking with
# opaque ENOSPC errors or from creating repo-root bead-named target directories.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

usage() {
    cat <<'EOF'
Usage:
  scripts/cargo_headroom.sh [options] <cargo-subcommand> [cargo-args...]

Options:
  --runner <rch|auto|local>   Cargo runner mode (default: KODE_CARGO_RUNNER or rch)
  --target-dir <path>         Override CARGO_TARGET_DIR for this invocation
  --tmpdir <path>             Override TMPDIR for this invocation
  --min-free-mb <mb>          Required free MB on target/tmp mounts (default: 24576)
  --min-inode-free-pct <pct>  Required free inode percent (default: 5)
  --max-local-cargo-processes <count>
                              Maximum local cargo/rustc processes before heavy
                              validation defers (default: 2)
  --admit-only                Emit the admission decision without running cargo
  --decision-json <path>      Also write the machine-readable decision to <path>
  --allow-local-fallback      Permit auto-mode local fallback for heavy commands
  --force-admit               Override local process pressure for this run
  -h, --help                  Show this help

Environment:
  KODE_CARGO_BUILD_ROOT         Build root used when CARGO_TARGET_DIR is unset
                              (default: /data/tmp/koompi_code_cli, or
                              /data/tmp/koompi_code_cli_cargo if the former
                              resolves inside this repository)
  KODE_CARGO_AGENT_SUFFIX       Per-agent subdirectory suffix (default: $USER)
  KODE_CARGO_ALLOW_REPO_TARGET  Set to 1 to allow target dirs under the repo root
  KODE_CARGO_ALLOW_LOCAL_FALLBACK
                              Set to 1 to permit heavy local fallback in auto mode
  KODE_CARGO_MAX_LOCAL_PROCESSES
                              Local cargo/rustc process cap for heavy gates
  KODE_CARGO_PROCESS_COUNT      Test/operator override for observed process count
  KODE_CARGO_FORCE_ADMIT        Set to 1 to override local process pressure
  KODE_CARGO_INCLUDE_SCRATCH_CLEANUP
                              Set to 1 to include scratch cleanup pressure on
                              allow decisions too; backoff/degraded decisions
                              include it automatically
  KODE_CARGO_SCRATCH_PLAN_JSON  Test/operator override containing planner JSON
EOF
}

die() {
    echo "[cargo-headroom] ERROR: $*" >&2
    exit 2
}

RUNNER="${KODE_CARGO_RUNNER:-rch}"
MIN_FREE_MB="${KODE_CARGO_HEADROOM_MIN_FREE_MB:-24576}"
MIN_INODE_FREE_PCT="${KODE_CARGO_HEADROOM_MIN_FREE_INODE_PCT:-5}"
MAX_LOCAL_CARGO_PROCESSES="${KODE_CARGO_MAX_LOCAL_PROCESSES:-2}"
RCH_QUEUE_FORECAST_MAX_AGE_SECS="${KODE_RCH_QUEUE_FORECAST_MAX_AGE_SECS:-120}"
DEFAULT_BUILD_ROOT="/data/tmp/koompi_code_cli"
if [[ -e "$DEFAULT_BUILD_ROOT" ]]; then
    if DEFAULT_BUILD_ROOT_REAL="$(cd "$DEFAULT_BUILD_ROOT" && pwd -P 2>/dev/null)"; then
        case "$DEFAULT_BUILD_ROOT_REAL" in
            "$PROJECT_ROOT"|"$PROJECT_ROOT"/*)
                DEFAULT_BUILD_ROOT="/data/tmp/koompi_code_cli_cargo"
                ;;
        esac
    fi
fi
BUILD_ROOT="${KODE_CARGO_BUILD_ROOT:-$DEFAULT_BUILD_ROOT}"
TARGET_OVERRIDE=""
TMPDIR_OVERRIDE=""
ADMIT_ONLY=0
DECISION_JSON_PATH=""
ALLOW_LOCAL_FALLBACK="${KODE_CARGO_ALLOW_LOCAL_FALLBACK:-0}"
FORCE_ADMIT="${KODE_CARGO_FORCE_ADMIT:-0}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --runner)
            [[ $# -ge 2 ]] || die "--runner requires a value"
            RUNNER="$2"
            shift 2
            ;;
        --target-dir)
            [[ $# -ge 2 ]] || die "--target-dir requires a value"
            TARGET_OVERRIDE="$2"
            shift 2
            ;;
        --tmpdir)
            [[ $# -ge 2 ]] || die "--tmpdir requires a value"
            TMPDIR_OVERRIDE="$2"
            shift 2
            ;;
        --min-free-mb)
            [[ $# -ge 2 ]] || die "--min-free-mb requires a value"
            MIN_FREE_MB="$2"
            shift 2
            ;;
        --min-inode-free-pct)
            [[ $# -ge 2 ]] || die "--min-inode-free-pct requires a value"
            MIN_INODE_FREE_PCT="$2"
            shift 2
            ;;
        --max-local-cargo-processes)
            [[ $# -ge 2 ]] || die "--max-local-cargo-processes requires a value"
            MAX_LOCAL_CARGO_PROCESSES="$2"
            shift 2
            ;;
        --admit-only)
            ADMIT_ONLY=1
            shift
            ;;
        --decision-json)
            [[ $# -ge 2 ]] || die "--decision-json requires a value"
            DECISION_JSON_PATH="$2"
            shift 2
            ;;
        --allow-local-fallback)
            ALLOW_LOCAL_FALLBACK=1
            shift
            ;;
        --force-admit)
            FORCE_ADMIT=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -V|--version)
            break
            ;;
        --)
            shift
            break
            ;;
        --*)
            die "unknown option: $1"
            ;;
        *)
            break
            ;;
    esac
done

[[ $# -gt 0 ]] || die "missing cargo subcommand; run with --help for usage"

case "$RUNNER" in
    rch|auto|local) ;;
    *) die "invalid runner '$RUNNER' (expected rch, auto, or local)" ;;
esac

[[ "$MIN_FREE_MB" =~ ^[0-9]+$ && "$MIN_FREE_MB" -gt 0 ]] \
    || die "invalid --min-free-mb '$MIN_FREE_MB'"
[[ "$MIN_INODE_FREE_PCT" =~ ^[0-9]+$ && "$MIN_INODE_FREE_PCT" -gt 0 && "$MIN_INODE_FREE_PCT" -lt 100 ]] \
    || die "invalid --min-inode-free-pct '$MIN_INODE_FREE_PCT'"
[[ "$MAX_LOCAL_CARGO_PROCESSES" =~ ^[0-9]+$ ]] \
    || die "invalid --max-local-cargo-processes '$MAX_LOCAL_CARGO_PROCESSES'"

safe_agent_suffix() {
    printf '%s' "${KODE_CARGO_AGENT_SUFFIX:-${USER:-agent}}" | tr -c 'A-Za-z0-9._-' '_'
}

resolve_dir() {
    local dir="$1"
    mkdir -p "$dir" || die "cannot create directory '$dir'"
    (cd "$dir" && pwd -P)
}

candidate_path() {
    local path="$1"
    local parent base

    if [[ "$path" != /* ]]; then
        path="$PROJECT_ROOT/$path"
    fi

    parent="$(dirname "$path")"
    base="$(basename "$path")"
    while [[ ! -d "$parent" && "$parent" != "/" ]]; do
        base="$(basename "$parent")/$base"
        parent="$(dirname "$parent")"
    done

    if [[ -d "$parent" ]]; then
        parent="$(cd "$parent" && pwd -P)"
        printf '%s/%s' "$parent" "$base"
    else
        printf '%s' "$path"
    fi
}

write_cache_tag() {
    local dir="$1"
    local tag="$dir/CACHEDIR.TAG"
    if [[ ! -e "$tag" ]]; then
        {
            echo "Signature: 8a477f597d28d172789f06886806bc55"
            echo "# This directory contains disposable Cargo build artifacts."
            echo "# See https://bford.info/cachedir/."
        } > "$tag"
    fi
}

json_escape() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    value="${value//$'\t'/\\t}"
    printf '%s' "$value"
}

forecast_not_checked() {
    printf '{"schema":"pi.cargo_headroom.rch_queue_forecast.v1","status":"not_checked","recommended_action":"run","reason":"not_checked","slot_pressure":"unknown","queue_depth":null,"active_builds":null,"queued_builds":null,"slots_available":null,"slots_total":null,"workers_healthy":null,"workers_total":null,"estimated_wait_seconds":null}'
}

RCH_QUEUE_FORECAST_JSON="$(forecast_not_checked)"
LOCAL_PROCESS_PRESSURE_JSON='{"schema":"pi.cargo_headroom.local_process_pressure.v1","status":"not_checked","recommended_action":"run","process_count":null,"max_processes":null,"force_override":false,"process_pattern":"cargo,rustc,cargo-clippy,clippy-driver","detail":"not_checked"}'

cargo_command_string() {
    local out="" arg
    for arg in "$@"; do
        if [[ -n "$out" ]]; then
            out+=" "
        fi
        out+="$arg"
    done
    printf '%s' "$out"
}

shell_join() {
    local out="" arg quoted
    for arg in "$@"; do
        if [[ -n "$out" ]]; then
            out+=" "
        fi
        printf -v quoted '%q' "$arg"
        out+="$quoted"
    done
    printf '%s' "$out"
}

planned_command_string() {
    local resolved_runner="$1"
    shift
    local cargo_text quoted_force quoted_target quoted_tmp rch_force

    cargo_text="$(shell_join cargo "$@")"
    case "$resolved_runner" in
        rch)
            rch_force="${RCH_FORCE_REMOTE:-true}"
            printf -v quoted_force '%q' "$rch_force"
            printf -v quoted_target '%q' "$CARGO_TARGET_DIR"
            printf -v quoted_tmp '%q' "$TMPDIR"
            printf 'env RCH_FORCE_REMOTE=%s CARGO_TARGET_DIR=%s TMPDIR=%s rch exec -- %s' \
                "$quoted_force" "$quoted_target" "$quoted_tmp" "$cargo_text"
            ;;
        local)
            printf -v quoted_target '%q' "$CARGO_TARGET_DIR"
            printf -v quoted_tmp '%q' "$TMPDIR"
            printf 'env CARGO_TARGET_DIR=%s TMPDIR=%s %s' "$quoted_target" "$quoted_tmp" "$cargo_text"
            ;;
        *)
            printf 'not_run: %s' "$cargo_text"
            ;;
    esac
}

json_field() {
    local json="$1"
    local field="$2"
    RCH_JSON="$json" RCH_FIELD="$field" python3 - <<'PY'
import json
import os

try:
    payload = json.loads(os.environ["RCH_JSON"])
except json.JSONDecodeError:
    print("")
else:
    value = payload.get(os.environ["RCH_FIELD"])
    print("" if value is None else value)
PY
}

local_process_pressure_json() {
    local count detail status recommended_action

    if [[ -n "${KODE_CARGO_PROCESS_COUNT:-}" ]]; then
        [[ "$KODE_CARGO_PROCESS_COUNT" =~ ^[0-9]+$ ]] \
            || die "invalid KODE_CARGO_PROCESS_COUNT '$KODE_CARGO_PROCESS_COUNT'"
        count="$KODE_CARGO_PROCESS_COUNT"
        detail="env_override"
    elif command -v pgrep >/dev/null 2>&1; then
        local raw
        if raw="$(pgrep -ax '(cargo|rustc|cargo-clippy|clippy-driver)' 2>/dev/null)"; then
            count="$(printf '%s\n' "$raw" | awk 'NF {n += 1} END {print n + 0}')"
            detail="$(printf '%s' "$raw" | head -n 8 | tr '\n' ';' | cut -c 1-240)"
        else
            count=0
            detail="no_matching_processes"
        fi
    else
        count=0
        status="unavailable"
        recommended_action="defer"
        detail="pgrep_not_found"
    fi

    if [[ -z "${status:-}" ]]; then
        if (( count > MAX_LOCAL_CARGO_PROCESSES )); then
            if [[ "$FORCE_ADMIT" == "1" ]]; then
                status="override"
                recommended_action="run"
            else
                status="high"
                recommended_action="defer"
            fi
        else
            status="ok"
            recommended_action="run"
        fi
    fi

    printf '{"schema":"pi.cargo_headroom.local_process_pressure.v1","status":"%s","recommended_action":"%s","process_count":%s,"max_processes":%s,"force_override":%s,"process_pattern":"cargo,rustc,cargo-clippy,clippy-driver","detail":"%s"}' \
        "$(json_escape "$status")" \
        "$(json_escape "$recommended_action")" \
        "$count" \
        "$MAX_LOCAL_CARGO_PROCESSES" \
        "$(if [[ "$FORCE_ADMIT" == "1" ]]; then echo true; else echo false; fi)" \
        "$(json_escape "$detail")"
}

admission_action_for_decision() {
    case "$1" in
        allow)
            printf 'allow'
            ;;
        degraded)
            printf 'fallback'
            ;;
        *)
            printf 'defer'
            ;;
    esac
}

scratch_cleanup_pressure_not_checked() {
    local reason="$1"
    printf '{"schema":"pi.cargo_headroom.scratch_cleanup_pressure.v1","status":"not_checked","recommended_action":"none","reason":"%s","source_kind":"none","cleanup_command_authorized":false,"destructive_actions_executed":false,"delete_apply_mode_available":false,"arg_max_safe_scan":null,"matched_entries":null,"listed_entries":null,"omitted_entries":null,"shallow_bytes":null,"by_cleanup_safety":{},"by_owner_marker_status":{},"risk_flags":{"arg_max_prone":false,"unknown_owner_entries":0,"active_owner_markers":0},"warnings":[],"operator_note":"scratch cleanup planner was not run"}' \
        "$(json_escape "$reason")"
}

scratch_cleanup_pressure_unavailable() {
    local reason="$1"
    local detail="$2"
    printf '{"schema":"pi.cargo_headroom.scratch_cleanup_pressure.v1","status":"unavailable","recommended_action":"manual_review","reason":"%s","detail":"%s","source_kind":"none","cleanup_command_authorized":false,"destructive_actions_executed":false,"delete_apply_mode_available":false,"arg_max_safe_scan":null,"matched_entries":null,"listed_entries":null,"omitted_entries":null,"shallow_bytes":null,"by_cleanup_safety":{},"by_owner_marker_status":{},"risk_flags":{"arg_max_prone":false,"unknown_owner_entries":0,"active_owner_markers":0},"warnings":[],"operator_note":"scratch cleanup planner did not run; do not infer cleanup safety"}' \
        "$(json_escape "$reason")" \
        "$(json_escape "$detail")"
}

summarize_scratch_cleanup_plan() {
    local raw="$1"
    local source_kind="$2"
    SCRATCH_PLAN_RAW="$raw" SCRATCH_PLAN_SOURCE_KIND="$source_kind" python3 - <<'PY'
import json
import os


def as_int(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return 0


raw = os.environ.get("SCRATCH_PLAN_RAW", "")
source_kind = os.environ.get("SCRATCH_PLAN_SOURCE_KIND", "unknown")
try:
    plan = json.loads(raw)
except json.JSONDecodeError as exc:
    print(json.dumps({
        "schema": "pi.cargo_headroom.scratch_cleanup_pressure.v1",
        "status": "malformed",
        "recommended_action": "manual_review",
        "reason": "planner_json_malformed",
        "detail": str(exc),
        "source_kind": source_kind,
        "cleanup_command_authorized": False,
        "destructive_actions_executed": False,
        "delete_apply_mode_available": False,
        "arg_max_safe_scan": None,
        "matched_entries": None,
        "listed_entries": None,
        "omitted_entries": None,
        "shallow_bytes": None,
        "by_cleanup_safety": {},
        "by_owner_marker_status": {},
        "risk_flags": {
            "arg_max_prone": False,
            "unknown_owner_entries": 0,
            "active_owner_markers": 0,
        },
        "warnings": [],
        "operator_note": "planner output was malformed; do not infer cleanup safety",
    }, sort_keys=True, separators=(",", ":")))
    raise SystemExit

if not isinstance(plan, dict):
    plan = {}
totals = plan.get("totals")
if not isinstance(totals, dict):
    totals = {}
by_cleanup_safety = totals.get("by_cleanup_safety")
if not isinstance(by_cleanup_safety, dict):
    by_cleanup_safety = {}
by_owner_marker_status = totals.get("by_owner_marker_status")
if not isinstance(by_owner_marker_status, dict):
    by_owner_marker_status = {}

matched_entries = as_int(totals.get("matched_entries"))
listed_entries = as_int(totals.get("listed_entries"))
omitted_entries = as_int(totals.get("omitted_entries"))
shallow_bytes = as_int(totals.get("shallow_bytes"))
unknown_owner_entries = (
    as_int(by_cleanup_safety.get("unknown_owner_fail_closed"))
    + as_int(by_cleanup_safety.get("unknown_owner_malformed_marker_fail_closed"))
)
active_owner_markers = as_int(by_owner_marker_status.get("active"))
arg_max_prone = matched_entries > 1000

status = "ok" if plan.get("schema") == "pi.scratch_cleanup_plan.v1" else "malformed"
unsafe_planner = bool(plan.get("destructive_actions_executed")) or bool(
    plan.get("delete_apply_mode_available")
)
if unsafe_planner:
    status = "unsafe"

if unsafe_planner:
    recommended_action = "backoff"
    reason = "planner_reported_destructive_capability"
elif active_owner_markers > 0:
    recommended_action = "preserve_active_targets"
    reason = "active_owner_markers_present"
elif matched_entries > 0:
    recommended_action = "manual_review"
    reason = "cleanup_candidates_need_approval"
else:
    recommended_action = "none"
    reason = "no_cleanup_candidates"

warnings = plan.get("warnings")
if not isinstance(warnings, list):
    warnings = []
warnings = [str(item) for item in warnings[:8]]

print(json.dumps({
    "schema": "pi.cargo_headroom.scratch_cleanup_pressure.v1",
    "status": status,
    "recommended_action": recommended_action,
    "reason": reason,
    "source_kind": source_kind,
    "planner_schema": plan.get("schema"),
    "owner_marker_schema": (
        plan.get("owner_marker_contract", {}).get("schema")
        if isinstance(plan.get("owner_marker_contract"), dict)
        else None
    ),
    "cleanup_command_authorized": False,
    "destructive_actions_executed": bool(plan.get("destructive_actions_executed")),
    "delete_apply_mode_available": bool(plan.get("delete_apply_mode_available")),
    "arg_max_safe_scan": plan.get("arg_max_safe_scan"),
    "matched_entries": matched_entries,
    "listed_entries": listed_entries,
    "omitted_entries": omitted_entries,
    "shallow_bytes": shallow_bytes,
    "by_cleanup_safety": by_cleanup_safety,
    "by_owner_marker_status": by_owner_marker_status,
    "risk_flags": {
        "arg_max_prone": arg_max_prone,
        "unknown_owner_entries": unknown_owner_entries,
        "active_owner_markers": active_owner_markers,
    },
    "warnings": warnings,
    "operator_note": plan.get(
        "operator_note",
        "scratch cleanup planner output is advisory only; do not infer cleanup safety",
    ),
}, sort_keys=True, separators=(",", ":")))
PY
}

scratch_cleanup_pressure_json() {
    local decision="$1"
    local raw

    if [[ "$decision" == "allow" && "${KODE_CARGO_INCLUDE_SCRATCH_CLEANUP:-0}" != "1" ]]; then
        scratch_cleanup_pressure_not_checked "admission_allowed"
        return 0
    fi

    if [[ -n "${KODE_CARGO_SCRATCH_PLAN_JSON:-}" ]]; then
        summarize_scratch_cleanup_plan "$KODE_CARGO_SCRATCH_PLAN_JSON" "env_override"
        return 0
    fi

    if ! command -v python3 >/dev/null 2>&1; then
        scratch_cleanup_pressure_unavailable "python3_not_found" ""
        return 0
    fi
    if [[ ! -f "$SCRIPT_DIR/plan_scratch_cleanup.py" ]]; then
        scratch_cleanup_pressure_unavailable "planner_not_found" "$SCRIPT_DIR/plan_scratch_cleanup.py"
        return 0
    fi
    if ! raw="$(python3 "$SCRIPT_DIR/plan_scratch_cleanup.py" --limit 0 --json 2>&1)"; then
        scratch_cleanup_pressure_unavailable "planner_failed" "$raw"
        return 0
    fi
    summarize_scratch_cleanup_plan "$raw" "planner"
}

is_safe_local_command() {
    local subcommand="$1"
    shift || true
    case "$subcommand" in
        fmt|metadata|fetch|tree|locate-project|-V|--version|version)
            return 0
            ;;
    esac

    return 1
}

build_rch_queue_forecast() {
    if ! command -v rch >/dev/null 2>&1; then
        printf '{"schema":"pi.cargo_headroom.rch_queue_forecast.v1","status":"unavailable","recommended_action":"backoff","reason":"rch_not_found","slot_pressure":"unknown","queue_depth":null,"active_builds":null,"queued_builds":null,"slots_available":null,"slots_total":null,"workers_healthy":null,"workers_total":null,"estimated_wait_seconds":null}'
        return 0
    fi

    local raw
    if ! raw="$(rch queue --json 2>&1)"; then
        RCH_QUEUE_RAW="$raw" python3 - <<'PY'
import json
import os

print(json.dumps({
    "schema": "pi.cargo_headroom.rch_queue_forecast.v1",
    "status": "unavailable",
    "recommended_action": "backoff",
    "reason": "queue_command_failed",
    "slot_pressure": "unknown",
    "queue_depth": None,
    "active_builds": None,
    "queued_builds": None,
    "slots_available": None,
    "slots_total": None,
    "workers_healthy": None,
    "workers_total": None,
    "estimated_wait_seconds": None,
    "detail": os.environ.get("RCH_QUEUE_RAW", "")[-240:],
}, sort_keys=True, separators=(",", ":")))
PY
        return 0
    fi

    RCH_QUEUE_RAW="$raw" RCH_QUEUE_FORECAST_MAX_AGE_SECS="$RCH_QUEUE_FORECAST_MAX_AGE_SECS" python3 - <<'PY'
from __future__ import annotations

import json
import math
import os
from datetime import datetime, timezone


def to_int(value):
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def parse_timestamp(value):
    if value is None:
        return None
    if isinstance(value, (int, float)):
        return datetime.fromtimestamp(value, timezone.utc)
    text = str(value)
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None


raw = os.environ.get("RCH_QUEUE_RAW", "")
try:
    payload = json.loads(raw)
except json.JSONDecodeError as exc:
    print(json.dumps({
        "schema": "pi.cargo_headroom.rch_queue_forecast.v1",
        "status": "malformed",
        "recommended_action": "backoff",
        "reason": "queue_json_malformed",
        "slot_pressure": "unknown",
        "queue_depth": None,
        "active_builds": None,
        "queued_builds": None,
        "slots_available": None,
        "slots_total": None,
        "workers_healthy": None,
        "workers_total": None,
        "estimated_wait_seconds": None,
        "detail": str(exc),
    }, sort_keys=True, separators=(",", ":")))
    raise SystemExit

data = payload.get("data") if isinstance(payload, dict) else None
if not isinstance(data, dict):
    print(json.dumps({
        "schema": "pi.cargo_headroom.rch_queue_forecast.v1",
        "status": "malformed",
        "recommended_action": "backoff",
        "reason": "queue_json_missing_data",
        "slot_pressure": "unknown",
        "queue_depth": None,
        "active_builds": None,
        "queued_builds": None,
        "slots_available": None,
        "slots_total": None,
        "workers_healthy": None,
        "workers_total": None,
        "estimated_wait_seconds": None,
    }, sort_keys=True, separators=(",", ":")))
    raise SystemExit

active = data.get("active_builds") if isinstance(data.get("active_builds"), list) else []
queued = data.get("queued_builds") if isinstance(data.get("queued_builds"), list) else []
queue_depth = to_int(data.get("queue_depth"))
if queue_depth is None:
    queue_depth = len(queued)
queued_builds = len(queued)
active_builds = len(active)
slots_available = to_int(data.get("slots_available"))
slots_total = to_int(data.get("slots_total"))
workers_healthy = to_int(data.get("workers_healthy"))
workers_total = to_int(data.get("workers_total"))
timestamp = parse_timestamp(data.get("timestamp") or payload.get("timestamp"))
max_age = to_int(os.environ.get("RCH_QUEUE_FORECAST_MAX_AGE_SECS")) or 120

if timestamp is None:
    status = "malformed"
    reason = "queue_timestamp_missing"
elif (datetime.now(timezone.utc) - timestamp).total_seconds() > max_age:
    status = "stale"
    reason = "queue_snapshot_stale"
else:
    status = "ok"
    reason = "queue_snapshot_ok"

slot_pressure = "unknown"
if slots_total and slots_total > 0 and slots_available is not None:
    used = max(0, slots_total - slots_available)
    ratio = used / slots_total
    if slots_available <= 0 and queue_depth > 0:
        slot_pressure = "saturated"
    elif ratio >= 0.90:
        slot_pressure = "saturated"
    elif ratio >= 0.75:
        slot_pressure = "high"
    elif ratio >= 0.50:
        slot_pressure = "moderate"
    else:
        slot_pressure = "low"

if status != "ok":
    recommended_action = "backoff"
elif slot_pressure == "saturated" and queue_depth > 0:
    recommended_action = "backoff"
    reason = "queue_saturated"
elif queue_depth > 0 or slot_pressure in {"high", "moderate"}:
    recommended_action = "split"
    reason = "queue_pressure"
else:
    recommended_action = "run"

estimated_wait_seconds = 0
if queue_depth > 0:
    available = slots_available if slots_available and slots_available > 0 else 1
    estimated_wait_seconds = int(max(60, math.ceil(queue_depth / available) * 60))

print(json.dumps({
    "schema": "pi.cargo_headroom.rch_queue_forecast.v1",
    "status": status,
    "recommended_action": recommended_action,
    "reason": reason,
    "slot_pressure": slot_pressure,
    "queue_depth": queue_depth,
    "active_builds": active_builds,
    "queued_builds": queued_builds,
    "slots_available": slots_available,
    "slots_total": slots_total,
    "workers_healthy": workers_healthy,
    "workers_total": workers_total,
    "estimated_wait_seconds": estimated_wait_seconds,
}, sort_keys=True, separators=(",", ":")))
PY
}

emit_admission_decision() {
    local decision="$1"
    local resolved_runner="$2"
    local reason="$3"
    local command_class="$4"
    local rch_detail="$5"
    shift 5
    local admission_action command_text json planned_command recommended_target_dir recommended_tmpdir scratch_pressure target_remediation tmpdir_remediation

    admission_action="$(admission_action_for_decision "$decision")"
    command_text="$(cargo_command_string "$@")"
    planned_command="$(planned_command_string "$resolved_runner" "$@")"
    recommended_target_dir="$BUILD_ROOT/$(safe_agent_suffix)/target"
    recommended_tmpdir="$BUILD_ROOT/$(safe_agent_suffix)/tmp"
    target_remediation="Set CARGO_TARGET_DIR or pass --target-dir to an off-repo scratch path such as $recommended_target_dir; current CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
    tmpdir_remediation="Set TMPDIR or pass --tmpdir to an off-repo scratch path such as $recommended_tmpdir; current TMPDIR=$TMPDIR"
    scratch_pressure="$(scratch_cleanup_pressure_json "$decision")"
    json="{\"schema\":\"pi.cargo_headroom.admission.v1\",\"decision\":\"$(json_escape "$decision")\",\"admission_action\":\"$(json_escape "$admission_action")\",\"requested_runner\":\"$(json_escape "$RUNNER")\",\"resolved_runner\":\"$(json_escape "$resolved_runner")\",\"reason\":\"$(json_escape "$reason")\",\"command_class\":\"$(json_escape "$command_class")\",\"allow_local_fallback\":$(if [[ "$ALLOW_LOCAL_FALLBACK" == "1" ]]; then echo true; else echo false; fi),\"force_override\":$(if [[ "$FORCE_ADMIT" == "1" ]]; then echo true; else echo false; fi),\"cargo_target_dir\":\"$(json_escape "$CARGO_TARGET_DIR")\",\"tmpdir\":\"$(json_escape "$TMPDIR")\",\"recommended_cargo_target_dir\":\"$(json_escape "$recommended_target_dir")\",\"recommended_tmpdir\":\"$(json_escape "$recommended_tmpdir")\",\"storage_remediation\":{\"cargo_target_dir\":\"$(json_escape "$target_remediation")\",\"tmpdir\":\"$(json_escape "$tmpdir_remediation")\"},\"cargo_command\":\"$(json_escape "$command_text")\",\"planned_command\":\"$(json_escape "$planned_command")\",\"local_process_pressure\":$LOCAL_PROCESS_PRESSURE_JSON,\"scratch_cleanup_pressure\":$scratch_pressure,\"rch_detail\":\"$(json_escape "$rch_detail")\",\"rch_queue_forecast\":$RCH_QUEUE_FORECAST_JSON}"

    echo "$json"
    if [[ -n "$DECISION_JSON_PATH" ]]; then
        printf '%s\n' "$json" > "$DECISION_JSON_PATH"
    fi
}

check_rch_health() {
    if ! command -v rch >/dev/null 2>&1; then
        RCH_DETAIL="rch_not_found"
        return 1
    fi
    if RCH_DETAIL="$(rch check --quiet 2>&1)"; then
        RCH_DETAIL="rch_check_ok"
        return 0
    fi
    if [[ -z "$RCH_DETAIL" ]]; then
        RCH_DETAIL="rch_check_failed"
    fi
    local forecast_status workers_healthy slots_available degraded_detail
    forecast_status="$(json_field "$RCH_QUEUE_FORECAST_JSON" status)"
    workers_healthy="$(json_field "$RCH_QUEUE_FORECAST_JSON" workers_healthy)"
    slots_available="$(json_field "$RCH_QUEUE_FORECAST_JSON" slots_available)"
    if [[ "$forecast_status" == "ok" \
        && "$workers_healthy" =~ ^[0-9]+$ && "$workers_healthy" -gt 0 \
        && "$slots_available" =~ ^[0-9]+$ && "$slots_available" -gt 0 ]]; then
        degraded_detail="$(printf '%s' "$RCH_DETAIL" | tr '\n' ' ' | cut -c 1-200)"
        RCH_DETAIL="rch_check_degraded_capacity_available: $degraded_detail"
        return 0
    fi
    return 1
}

if [[ -n "$TARGET_OVERRIDE" ]]; then
    export CARGO_TARGET_DIR="$TARGET_OVERRIDE"
elif [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="$BUILD_ROOT/$(safe_agent_suffix)/target"
fi

if [[ -n "$TMPDIR_OVERRIDE" ]]; then
    export TMPDIR="$TMPDIR_OVERRIDE"
elif [[ -z "${TMPDIR:-}" ]]; then
    export TMPDIR="$BUILD_ROOT/$(safe_agent_suffix)/tmp"
fi

TARGET_CANDIDATE="$(candidate_path "$CARGO_TARGET_DIR")"
case "$TARGET_CANDIDATE" in
    "$PROJECT_ROOT"/*)
        if [[ "${KODE_CARGO_ALLOW_REPO_TARGET:-0}" != "1" ]]; then
            die "CARGO_TARGET_DIR is under the repo root ($TARGET_CANDIDATE). Use /data/tmp or set KODE_CARGO_ALLOW_REPO_TARGET=1 explicitly."
        fi
        ;;
esac

case "$TARGET_CANDIDATE" in
    "$PROJECT_ROOT"/bd-*|"$PROJECT_ROOT"/bd-*/*)
        die "refusing bead-named repo-root target dir '$TARGET_CANDIDATE'; use an absolute off-repo CARGO_TARGET_DIR"
        ;;
esac

TMPDIR_CANDIDATE="$(candidate_path "$TMPDIR")"
case "$TMPDIR_CANDIDATE" in
    "$PROJECT_ROOT"/*)
        if [[ "${KODE_CARGO_ALLOW_REPO_TARGET:-0}" != "1" ]]; then
            die "TMPDIR is under the repo root ($TMPDIR_CANDIDATE). Use /data/tmp or set KODE_CARGO_ALLOW_REPO_TARGET=1 explicitly."
        fi
        ;;
esac

CARGO_TARGET_DIR="$(resolve_dir "$CARGO_TARGET_DIR")"
TMPDIR="$(resolve_dir "$TMPDIR")"
export CARGO_TARGET_DIR TMPDIR

case "$CARGO_TARGET_DIR" in
    "$PROJECT_ROOT"/*)
        if [[ "${KODE_CARGO_ALLOW_REPO_TARGET:-0}" != "1" ]]; then
            die "CARGO_TARGET_DIR is under the repo root ($CARGO_TARGET_DIR). Use /data/tmp or set KODE_CARGO_ALLOW_REPO_TARGET=1 explicitly."
        fi
        ;;
esac

case "$CARGO_TARGET_DIR" in
    "$PROJECT_ROOT"/bd-*|"$PROJECT_ROOT"/bd-*/*)
        die "refusing bead-named repo-root target dir '$CARGO_TARGET_DIR'; use an absolute off-repo CARGO_TARGET_DIR"
        ;;
esac

write_cache_tag "$CARGO_TARGET_DIR"

COMMAND_CLASS="heavy"
if is_safe_local_command "$@"; then
    COMMAND_CLASS="safe_local"
fi
LOCAL_PROCESS_PRESSURE_JSON="$(local_process_pressure_json)"
LOCAL_PROCESS_RECOMMENDED_ACTION="$(json_field "$LOCAL_PROCESS_PRESSURE_JSON" recommended_action)"
LOCAL_PROCESS_STATUS="$(json_field "$LOCAL_PROCESS_PRESSURE_JSON" status)"

HEADROOM_OK=1
HEADROOM_FAILURES=()

probe_headroom() {
    local label="$1"
    local path="$2"
    local disk_row avail_kb mount_point avail_mb inode_used_pct inode_free_pct

    disk_row="$(df -Pk "$path" | awk 'NR==2 {print $4 "|" $6}')"
    [[ -n "$disk_row" ]] || die "unable to read disk stats for $label path '$path'"

    avail_kb="${disk_row%%|*}"
    mount_point="${disk_row#*|}"
    avail_mb=$((avail_kb / 1024))

    inode_used_pct="$(df -Pi "$path" | awk 'NR==2 {gsub(/%/, "", $5); print $5}')"
    [[ -n "$inode_used_pct" ]] || inode_used_pct=100
    inode_free_pct=$((100 - inode_used_pct))

    echo "[cargo-headroom] $label mount=$mount_point free=${avail_mb}MB inode_free=${inode_free_pct}% path=$path"

    if (( avail_mb < MIN_FREE_MB )); then
        HEADROOM_FAILURES+=("$label mount '$mount_point' has ${avail_mb}MB free (< ${MIN_FREE_MB}MB required)")
        return 1
    fi
    if (( inode_free_pct < MIN_INODE_FREE_PCT )); then
        HEADROOM_FAILURES+=("$label mount '$mount_point' has ${inode_free_pct}% free inodes (< ${MIN_INODE_FREE_PCT}% required)")
        return 1
    fi
}

probe_headroom "cargo_target" "$CARGO_TARGET_DIR" || HEADROOM_OK=0
probe_headroom "tmp" "$TMPDIR" || HEADROOM_OK=0

echo "[cargo-headroom] runner=$RUNNER cargo_target=$CARGO_TARGET_DIR tmp=$TMPDIR"

if (( HEADROOM_OK == 0 )); then
    emit_admission_decision \
        "backoff" \
        "none" \
        "insufficient_headroom" \
        "blocked" \
        "$(cargo_command_string "${HEADROOM_FAILURES[@]}")" \
        "$@"
    printf '[cargo-headroom] ERROR: %s\n' "${HEADROOM_FAILURES[@]}" >&2
    exit 2
fi

run_with_rch() {
    exec env \
        RCH_FORCE_REMOTE="${RCH_FORCE_REMOTE:-true}" \
        CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
        TMPDIR="$TMPDIR" \
        rch exec -- cargo "$@"
}

run_local_cargo() {
    exec env CARGO_TARGET_DIR="$CARGO_TARGET_DIR" TMPDIR="$TMPDIR" cargo "$@"
}

RCH_QUEUE_FORECAST_JSON="$(build_rch_queue_forecast)"
RCH_QUEUE_FORECAST_ACTION="$(json_field "$RCH_QUEUE_FORECAST_JSON" recommended_action)"
RCH_QUEUE_FORECAST_REASON="$(json_field "$RCH_QUEUE_FORECAST_JSON" reason)"

case "$RUNNER" in
    rch)
        if ! check_rch_health; then
            emit_admission_decision "backoff" "none" "rch_unavailable" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            exit 2
        fi
        if [[ "$COMMAND_CLASS" == "heavy" && "$LOCAL_PROCESS_RECOMMENDED_ACTION" == "defer" ]]; then
            emit_admission_decision "backoff" "none" "local_process_pressure" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            exit 2
        fi
        if [[ "$COMMAND_CLASS" == "heavy" && "$RCH_QUEUE_FORECAST_ACTION" == "backoff" ]]; then
            emit_admission_decision "backoff" "none" "rch_${RCH_QUEUE_FORECAST_REASON}" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            exit 2
        fi
        emit_admission_decision "allow" "rch" "rch_available" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
        if (( ADMIT_ONLY == 1 )); then
            exit 0
        fi
        run_with_rch "$@"
        ;;
    auto)
        if check_rch_health; then
            if [[ "$COMMAND_CLASS" == "heavy" && "$LOCAL_PROCESS_RECOMMENDED_ACTION" == "defer" ]]; then
                emit_admission_decision "backoff" "none" "local_process_pressure" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
                exit 2
            fi
            if [[ "$COMMAND_CLASS" == "heavy" && "$RCH_QUEUE_FORECAST_ACTION" == "backoff" ]]; then
                emit_admission_decision "backoff" "none" "rch_${RCH_QUEUE_FORECAST_REASON}" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
                exit 2
            fi
            emit_admission_decision "allow" "rch" "rch_available" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            if (( ADMIT_ONLY == 1 )); then
                exit 0
            fi
            run_with_rch "$@"
        fi
        if [[ "$COMMAND_CLASS" == "safe_local" ]]; then
            emit_admission_decision "degraded" "local" "safe_local_command" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            if (( ADMIT_ONLY == 1 )); then
                exit 0
            fi
            run_local_cargo "$@"
        fi
        if [[ "$COMMAND_CLASS" == "heavy" && "$LOCAL_PROCESS_RECOMMENDED_ACTION" == "defer" ]]; then
            emit_admission_decision "backoff" "none" "local_process_pressure" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            exit 2
        fi
        if [[ "$ALLOW_LOCAL_FALLBACK" == "1" ]]; then
            emit_admission_decision "degraded" "local" "explicit_local_fallback" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
            if (( ADMIT_ONLY == 1 )); then
                exit 0
            fi
            run_local_cargo "$@"
        fi
        emit_admission_decision "backoff" "none" "rch_unavailable" "$COMMAND_CLASS" "$RCH_DETAIL" "$@"
        exit 2
        ;;
    local)
        if [[ "$COMMAND_CLASS" == "heavy" && "$LOCAL_PROCESS_RECOMMENDED_ACTION" == "defer" ]]; then
            emit_admission_decision "backoff" "none" "local_process_pressure" "$COMMAND_CLASS" "not_checked" "$@"
            exit 2
        fi
        emit_admission_decision "allow" "local" "explicit_local_runner" "$COMMAND_CLASS" "not_checked" "$@"
        if (( ADMIT_ONLY == 1 )); then
            exit 0
        fi
        run_local_cargo "$@"
        ;;
esac
