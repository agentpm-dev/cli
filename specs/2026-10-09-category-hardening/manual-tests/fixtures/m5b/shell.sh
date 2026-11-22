# Source the generated paths.sh; it loads this copy for the current owned run.
m5b_fixture() {
  "$M5B_PYTHON" -B "$M5B_FIXTURE" "$1" "$M5B_ROOT" "${@:2}"
}
m5b_ref() {
  "$M5B_PYTHON" -c 'import json,sys; print(json.load(open(sys.argv[1]))["refs"][sys.argv[2]])' "$M5B_ROOT/session.json" "$1"
}
m5b() {
  [[ "$M5B_BASE" == 'http://127.0.0.1:5000' ]] || return 1
  env AGENTPM_TOKEN="${M5B_TOKEN:?Set your LOCAL PAT}" AGENTPM_TELEMETRY=0 NO_COLOR=1 \
    "$M5B_APM" --base-url "$M5B_BASE" "$@"
}
m5b_fault() {
  local m5b_case="$1" m5b_origin
  shift
  m5b_origin="$("$M5B_PYTHON" -c 'import json,sys; from urllib.parse import urlsplit; v=json.load(open(sys.argv[1])); u=urlsplit(v["base_url"]); assert v["surface"]=="controlled loopback" and u.scheme=="http" and u.hostname=="127.0.0.1" and u.port and u.port!=5000; print(v["base_url"])' "$M5B_ROOT/control-server.json")" || return 1
  env AGENTPM_TOKEN=apm_m5b.fixture AGENTPM_TELEMETRY=0 NO_COLOR=1 \
    "$M5B_APM" --base-url "$m5b_origin/$m5b_case" "$@"
}
m5b_expect() {
  local m5b_label="$1" m5b_expected="$2" m5b_actual
  shift 2
  m5b_fixture begin "$m5b_label" || return 1
  pwd > "$M5B_ROOT/evidence/$m5b_label.cwd.txt"
  printf '%q ' "$@" > "$M5B_ROOT/evidence/$m5b_label.command.txt"
  printf '\n' >> "$M5B_ROOT/evidence/$m5b_label.command.txt"
  if "$@" > "$M5B_ROOT/evidence/$m5b_label.stdout.txt" \
           2> "$M5B_ROOT/evidence/$m5b_label.stderr.txt"; then
    m5b_actual=0
  else
    m5b_actual=$?
  fi
  cat "$M5B_ROOT/evidence/$m5b_label.stdout.txt"
  cat "$M5B_ROOT/evidence/$m5b_label.stderr.txt" >&2
  printf '{"exit_code":%s,"expected_exit_code":%s}\n' "$m5b_actual" "$m5b_expected" \
    > "$M5B_ROOT/evidence/$m5b_label.result.json"
  printf 'ACTUAL exit %s | EXPECTED exit %s | %s\n' "$m5b_actual" "$m5b_expected" "$m5b_label"
  if [[ "$m5b_actual" != "$m5b_expected" ]]; then
    printf 'STOP: unexpected result; inspect evidence before continuing.\n' >&2
    return 1
  fi
}
