#!/usr/bin/env bash
# Installs Cledit on the Windows runner the way a user would and checks the result, so a
# broken installer or update stops the release. Called by release.sh:
#   check-install.sh fresh <Setup.exe> <version>
#   check-install.sh update <earlier Setup.exe> <update_probe.exe> <version> [Setup.exe]
# Without the last Setup.exe, the update comes from the latest GitHub release.
set -Eeuo pipefail
# The run's annotations are what can be read back without its logs, so failures go there.
trap 'echo "::error::${BASH_SOURCE[0]}:${LINENO}: ${BASH_COMMAND} failed"' ERR
export MSYS_NO_PATHCONV=1

# Must match AppId in installer/cledit.iss.
uninstall_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{C150D3A9-C758-4C99-A7A4-27CB8B72382C}_is1'
app="$(cygpath -u "$LOCALAPPDATA")/Programs/Cledit"
start_menu="$(cygpath -u "$APPDATA")/Microsoft/Windows/Start Menu/Programs/Cledit.lnk"
desktop="$(cygpath -u "$(powershell -NoProfile -Command "[Environment]::GetFolderPath('Desktop')" | tr -d '\r')")/Cledit.lnk"
failures=0

fail() {
  echo "::error::$*"
  failures=$((failures + 1))
}

stop_cledit() {
  taskkill /F /IM cledit.exe >/dev/null 2>&1 || true
}

uninstall() {
  stop_cledit
  timeout 300 "$app/unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART || return 1
  # The uninstaller finishes from a copy of itself in the temporary folder.
  for _ in $(seq 60); do
    [ -f "$app/cledit.exe" ] || return 0
    sleep 2
  done
  return 1
}

# Tasks as named in installer/cledit.iss; the ones left out are unchecked.
install() {
  echo "Installing $1 with $2"
  if [ -f "$app/unins000.exe" ]; then
    uninstall || true
  fi
  rm -rf "$app"
  timeout 600 "$1" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART "/TASKS=$2" \
    || { echo "::error::$1 failed"; exit 1; }
  [ -f "$app/cledit.exe" ] || { echo "::error::no cledit.exe in $app"; exit 1; }
}

check_version() {
  local installed
  installed=$(reg query "$uninstall_key" /v DisplayVersion | tr -d '\r' | awk '/DisplayVersion/ { print $3 }')
  [ "$installed" = "$1" ] || fail "the installed version is ${installed:-missing}, not $1"
}

# Every DLL that Cledit or a DLL it ships loads must be next to it or part of Windows. The
# Visual C++ runtime counts as missing even when the runner has it, since users may not.
check_dependencies() {
  local vs dumpbin shipped checked=0
  vs=$(cygpath -u "$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" \
    -latest -products '*' -property installationPath | tr -d '\r')")
  dumpbin=$(ls "$vs"/VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe | sort -V | tail -1)
  shipped=$(ls "$app" | tr '[:upper:]' '[:lower:]')
  for binary in "$app"/*.exe "$app"/*.dll; do
    local dlls
    dlls=$("$dumpbin" -nologo -dependents "$(cygpath -w "$binary")" | tr -d '\r' \
      | sed -n 's/^ \{4,\}\([A-Za-z0-9_.-]*\.[dD][lL][lL]\)$/\1/p' | tr '[:upper:]' '[:lower:]' | sort -u)
    if [ "$(basename "$binary")" = cledit.exe ] && ! grep -qx vcruntime140.dll <<<"$dlls"; then
      fail "reading cledit.exe's DLLs found no vcruntime140.dll, so the DLL check is not working"
    fi
    for dll in $dlls; do
      checked=$((checked + 1))
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
  echo "::notice::checked $checked DLLs loaded by $(ls "$app"/*.exe "$app"/*.dll | wc -l) installed files"
}

check_file_type() {
  reg query 'HKCU\Software\Classes\.cledit' /ve | grep -q Cledit.Project \
    || fail ".cledit files are not registered to Cledit"
  reg query 'HKCU\Software\Classes\.editproj' /ve | grep -q Cledit.Project \
    || fail ".editproj files are not registered to Cledit"
  reg query 'HKCU\Software\Classes\Cledit.Project\shell\open\command' /ve | grep -qi 'cledit.exe' \
    || fail "opening a project does not start cledit.exe"
}

# Whether a shortcut is there should match whether its task was checked.
check_shortcut() {
  local name=$1 path=$2 wanted=$3
  if [ "$wanted" = yes ] && [ ! -f "$path" ]; then
    fail "there is no $name shortcut"
  elif [ "$wanted" = no ] && [ -f "$path" ]; then
    fail "there is a $name shortcut that was not asked for"
  fi
}

# The runner has no graphics card, so a window that fails to open is reported, not failed.
check_starts() {
  "$app/cledit.exe" &
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
  uninstall || { fail "uninstalling failed or left $app/cledit.exe"; return; }
  if reg query 'HKCU\Software\Classes\.cledit' /ve >/dev/null 2>&1; then
    fail "uninstalling left .cledit registered"
  fi
  if reg query "$uninstall_key" >/dev/null 2>&1; then
    fail "uninstalling left Cledit in Apps"
  fi
  check_shortcut "Start menu" "$start_menu" no
  check_shortcut desktop "$desktop" no
}

case "${1:-}" in
  fresh)
    install "$2" startmenuicon,desktopicon,fileassoc
    check_version "$3"
    check_dependencies
    check_file_type
    check_shortcut "Start menu" "$start_menu" yes
    check_shortcut desktop "$desktop" yes
    check_starts
    check_uninstall
    ;;
  update)
    # Choices unlike the installer's defaults, so the update is seen keeping them.
    install "$2" desktopicon,fileassoc
    "$3" "$4" ${5:+"$(cygpath -w "$5")"} || fail "updating to $4 failed"
    check_version "$4"
    check_dependencies
    check_file_type
    check_shortcut "Start menu" "$start_menu" no
    check_shortcut desktop "$desktop" yes
    check_uninstall
    ;;
  *)
    echo "usage: $0 fresh <Setup.exe> <version> | update <earlier Setup.exe> <update_probe.exe> <version> [Setup.exe]" >&2
    exit 2
    ;;
esac

if [ "$failures" -gt 0 ]; then
  echo "::error::$failures install check(s) failed"
  exit 1
fi
echo "::notice::install check passed: $1 $([ "$1" = update ] && echo "$2 to $4" || echo "$3")"
