#!/usr/bin/env bash
# Builds Cledit, packs its installer with Velopack into ./Releases, and publishes it as a
# GitHub release when PUBLISH is "true". Runs on the Windows runner from the release workflow;
# it lives here rather than in the workflow so packaging changes need no workflow edit.
set -euo pipefail

# Must match the velopack crate in Cargo.toml: the app and its installer speak one format.
VPK_VERSION=1.2.161
# FFmpeg 8 is what ffmpeg-sys-next 9 binds; the GPL build carries x264 and x265.
FFMPEG_BUILD=ffmpeg-n8.1-latest-win64-gpl-shared-8.1
REPO_URL=https://github.com/C1art/edit
: "${RUNNER_TEMP:?run this on the release workflow runner}"
: "${GITHUB_TOKEN:?}"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
echo "Cledit $version"
if [ "${GITHUB_REF_TYPE:-}" = tag ] && [ "${GITHUB_REF_NAME:-}" != "v$version" ]; then
  echo "::error::tag $GITHUB_REF_NAME does not match the Cargo version $version"
  exit 1
fi

echo "::group::FFmpeg"
ffmpeg="$RUNNER_TEMP/$FFMPEG_BUILD"
curl -sSfL -o "$RUNNER_TEMP/ffmpeg.zip" \
  "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/$FFMPEG_BUILD.zip"
rm -rf "$ffmpeg"
unzip -q "$RUNNER_TEMP/ffmpeg.zip" -d "$RUNNER_TEMP"
ffmpeg_version=$(sed -n 's/.*FFMPEG_VERSION "\(.*\)"/\1/p' "$ffmpeg/include/libavutil/ffversion.h")
echo "FFmpeg $ffmpeg_version"
echo "::endgroup::"

echo "::group::Build"
FFMPEG_DIR=$(cygpath -w "$ffmpeg") LIBCLANG_PATH='C:\Program Files\LLVM\bin' \
  cargo build --release --locked -p app --bin cledit --example update_probe
echo "::endgroup::"

echo "::group::Gather the files to install"
rm -rf dist
mkdir dist
cp target/release/cledit.exe LICENSE dist/
cp "$ffmpeg"/bin/*.dll dist/
# The Visual C++ runtime goes next to the exe instead of being installed system-wide: its
# own installer needs admin rights and can fail (it crashes Windows Sandbox), and the DLLs
# are Microsoft's to redistribute this way. Taken from the Visual Studio that built the exe,
# so they are at least as new as it needs.
vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe"
vs=$(cygpath "$("$vswhere" -latest -products '*' -property installationPath | tr -d '\r')")
crt=$(ls -d "$vs"/VC/Redist/MSVC/*/x64/Microsoft.VC14*.CRT | sort -V | tail -1)
echo "Visual C++ runtime from $crt"
cp "$crt"/*.dll dist/
{
  cat THIRD-PARTY-NOTICES.md
  echo
  echo "## FFmpeg in this release"
  echo
  echo "FFmpeg $ffmpeg_version, from $FFMPEG_BUILD.zip of BtbN/FFmpeg-Builds."
  commit=${ffmpeg_version##*-g}
  echo "Source: https://github.com/FFmpeg/FFmpeg/tree/${commit%%-*}"
} > dist/THIRD-PARTY-NOTICES.md
icon=$(ls target/release/build/app-*/out/cledit.ico | head -1)
ls -l dist
echo "::endgroup::"

echo "::group::Release notes"
notes="$RUNNER_TEMP/release-notes.md"
if [ -f "docs/releases/$version.md" ]; then
  cp "docs/releases/$version.md" "$notes"
else
  previous=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || true)
  git log --format='%s' ${previous:+$previous..}HEAD \
    | { grep -E '^(feat|fix|perf)(\(|!|:)' || true; } | sed 's/^/- /' > "$notes"
fi
cat "$notes"
echo "::endgroup::"

echo "::group::Pack"
dotnet tool install -g vpk --version "$VPK_VERSION" || dotnet tool update -g vpk --version "$VPK_VERSION"
export PATH="$PATH:$HOME/.dotnet/tools"
# Earlier releases let vpk make delta packages; the first release has none.
vpk download github --repoUrl "$REPO_URL" --token "$GITHUB_TOKEN" || true
vpk pack --packId Cledit --packVersion "$version" --packDir dist \
  --mainExe cledit.exe --packTitle Cledit --packAuthors Clart \
  --icon "$icon" --releaseNotes "$notes"
ls -l Releases
echo "::endgroup::"

echo "::group::Install check"
bash scripts/check-install.sh fresh Releases/Cledit-win-Setup.exe "$version"
echo "::endgroup::"

# The update check installs the latest published release and updates it to this build: from
# the local feed before publishing, or from GitHub, as users get it, once published.
previous=$(curl -sSf -H "Authorization: Bearer $GITHUB_TOKEN" \
  "https://api.github.com/repos/C1art/edit/releases/latest" | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p')
previous_setup=""
if [ -n "$previous" ] && [ "$previous" != "$version" ]; then
  previous_setup="$RUNNER_TEMP/Cledit-$previous-Setup.exe"
  curl -sSfL -o "$previous_setup" "$REPO_URL/releases/download/v$previous/Cledit-win-Setup.exe"
else
  echo "::notice::no earlier release to update from (latest is ${previous:-none})"
fi
probe=target/release/examples/update_probe.exe

if [ "${PUBLISH:-false}" = true ]; then
  vpk upload github --repoUrl "$REPO_URL" --token "$GITHUB_TOKEN" --publish \
    --releaseName "Cledit $version" --tag "v$version" --targetCommitish "${GITHUB_SHA:?}"
  feed=""
else
  feed=Releases
fi

if [ -n "$previous_setup" ]; then
  echo "::group::Update check from $previous"
  bash scripts/check-install.sh update "$previous_setup" "$probe" "$version" $feed
  echo "::endgroup::"
fi
