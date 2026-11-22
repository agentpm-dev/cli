# Loaded by this owned run's generated paths.sh.
m6a_fixture() {
  "$M6A_PYTHON" -B "$M6A_FIXTURE" "$1" "$M6A_ROOT" "${@:2}"
}
m6a() {
  m6a_fixture guard "$PWD" || return 1
  env -u OPENAI_API_KEY -u ANTHROPIC_API_KEY \
    AGENTPM_TELEMETRY=0 AGENTPM_TOKEN=apm_m6a.fixture NO_COLOR=1 \
    XDG_CONFIG_HOME="$M6A_ROOT/config" XDG_CACHE_HOME="$M6A_ROOT/cache" \
    APPDATA="$M6A_ROOT/config" LOCALAPPDATA="$M6A_ROOT/cache" \
    "$M6A_APM" --base-url http://127.0.0.1:9 "$@"
}
m6a_machine() {
  m6a_fixture machine "$PWD"
}
m6a_expect() {
  local m6a_label="$1" m6a_expected="$2" m6a_actual
  shift 2
  m6a_fixture begin "$m6a_label" "$m6a_expected" "$PWD" "$@" || return 1
  if "$@" > "$M6A_ROOT/evidence/$m6a_label.stdout.txt" \
           2> "$M6A_ROOT/evidence/$m6a_label.stderr.txt"; then
    m6a_actual=0
  else
    m6a_actual=$?
  fi
  cat "$M6A_ROOT/evidence/$m6a_label.stdout.txt"
  cat "$M6A_ROOT/evidence/$m6a_label.stderr.txt" >&2
  printf 'ACTUAL exit %s | EXPECTED exit %s | %s\n' "$m6a_actual" "$m6a_expected" "$m6a_label"
  m6a_fixture finish "$m6a_label" "$m6a_actual"
}
