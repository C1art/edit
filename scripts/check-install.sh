#!/usr/bin/env bash
# Installs Cledit on the Windows runner the way a user would and checks the result, so a
# broken installer or update stops the release. Called by release.sh:
#   check-install.sh fresh <Setup.exe> <version>
#   check-install.sh update <earlier Setup.exe> <update_probe.exe> <version> [feed folder]
set -Eeuo pipefail
# The run's annotations are what can be read back without its logs, so failures go there.
trap 'echo "::error::${BASH_SOURCE[0]}:${LINENO}: ${BASH_COMMAND} failed"' ERR
export MSYS_NO_PATHCONV=1

root="$(cygpath -u "$LOCALAPPDATA")/Cledit"
current="$root/current"
failures=0

fail() {
  echo "::error::$*"
  failures=$((failures + 1))
}

stop_cledit() {
  taskkill /F /IM cledit.exe >/dev/null 2>&1 || true
}

install() {
  echo "Installing $1"
  stop_cledit
  rm -rf "$root"
  timeout 600 "$1" --silent
  # Setup may start Cledit when it finishes.
  sleep 5
  stop_cledit
  [ -f "$current/cledit.exe" ] || { echo "::error::no cledit.exe in $current"; exit 1; }
}

check_version() {
  grep -q "<version>$1</version>" "$current/sq.version" \
    || fail "the installed version is not $1: $(cat "$current/sq.version")"
}

# Every DLL that Cledit or a DLL it ships loads must be next to it or part of Windows. The
# Visual C++ runtime counts as missing even when the runner has it, since users may not.
check_dependencies() {
  local vs dumpbin shipped
  vs=$(cygpath -u "$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" \
    -latest -products '*' -property installationPath | tr -d '\r')")
  dumpbin=$(ls "$vs"/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe | sort -V | tail -1)
  shipped=$(ls "$current" | tr '[:upper:]' '[:lower:]')
  for binary in "$current"/*.exe "$current"/*.dll; do
    for dll in $("$dumpbin" -nologo -dependents "$(cygpath -w "$binary")" | tr -d '\r' \
      | sed -n 's/^ \{4,\}\([A-Za-z0-9_.-]*\.[dD][lL][lL]\)$/\1/p' | tr '[:upper:]' '[:lower:]' | sort -u); do
      if grep -qx "$dll" <<<"$shipped"; then
        continue
      elif [[ $dll =~ ^(vcruntime|msvcp|vcomp|concrt|vccorlib) ]]; then
        fail "$(basename "$binary") needs $dll, which is not installed with Cledit"
      elif [[ $dll =~ ^(api|ext)-ms- ]] || [ -f "/c/Windows/System32/$dll" ]; then
        continue
      else
        fail "$(basename "$binary") needs $dll, which is neither installed nor part of Windows"
      fi
    done
  done
  echo "::notice::checked the DLLs that $(ls "$current"/*.exe "$current"/*.dll | wc -l) installed files load"
}

check_file_type() {
  reg query 'HKCU\Software\Classes\.cledit' /ve | grep -q Cledit.Project \
    || fail ".cledit files are not registered to Cledit"
  reg query 'HKCU\Software\Classes\.editproj' /ve | grep -q Cledit.Project \
    || fail ".editproj files are not registered to Cledit"
  reg query 'HKCU\Software\Classes\Cledit.Project\shell\open\command' /ve | grep -qi 'cledit.exe' \
    || fail "opening a project does not start cledit.exe"
}

# The runner has no graphics card, so a window that fails to open is reported, not failed.
check_starts() {
  "$current/cledit.exe" &
  local pid=$!
  sleep 20
  if kill -0 "$pid" 2>/dev/null; then
    echo "::notice::Cledit was still running 20 seconds after starting"
    stop_cledit
  else
    local status=0
    wait "$pid" || status=$?
    echo "::warning::Cledit exited within 20 seconds with status $status"
  fi
}

check_uninstall() {
  stop_cledit
  if ! timeout 300 "$root/Update.exe" --silent uninstall; then
    fail "uninstalling failed"
    return
  fi
  if reg query 'HKCU\Software\Classes\.cledit' /ve >/dev/null 2>&1; then
    fail "uninstalling left .cledit registered"
  fi
  [ ! -f "$current/cledit.exe" ] || fail "uninstalling left $current/cledit.exe"
}

case "${1:-}" in
  fresh)
    install "$2"
    check_version "$3"
    check_dependencies
    check_file_type
    check_starts
    check_uninstall
    ;;
  update)
    install "$2"
    cp "$3" "$current/"
    "$current/$(basename "$3")" "$4" ${5:+"$(cygpath -w "$5")"}
    echo "Waiting for the update to $4 to apply"
    for _ in $(seq 60); do
      grep -q "<version>$4</version>" "$current/sq.version" 2>/dev/null && break
      sleep 3
    done
    check_version "$4"
    check_dependencies
    check_file_type
    ;;
  *)
    echo "usage: $0 fresh <Setup.exe> <version> | update <earlier Setup.exe> <update_probe.exe> <version> [feed folder]" >&2
    exit 2
    ;;
esac

if [ "$failures" -gt 0 ]; then
  echo "::error::$failures install check(s) failed"
  exit 1
fi
echo "::notice::install check passed: $1 $([ "$1" = update ] && echo "$2 to $4" || echo "$3")"
