#!/bin/bash
set -u
set -o pipefail

# S0 evidence capture. Read-only: no sudo, networksetup -set*, route add/delete,
# scutil --set, DNS changes, or service restarts. Run on a disposable test Mac.
umask 077
root="${1:-$PWD/mac-s0-evidence}"
if [ "$(uname -s)" != "Darwin" ]; then
  printf 'This audit requires macOS. No network settings were changed.\n' >&2
  exit 2
fi
if [ -e "$root" ]; then
  printf 'Evidence directory already exists: %s\nChoose a new path to avoid overwriting a prior snapshot.\n' "$root" >&2
  exit 2
fi
mkdir -p "$root" || exit 1
chmod 700 "$root"

capture() {
  local filename="$1"; shift
  {
    printf 'command:'
    printf ' %q' "$@"
    printf '\n'
    "$@"
    printf '\nexit_code=%s\n' "$?"
  } > "$root/$filename" 2>&1
}

capture os.txt sw_vers
capture architecture.txt uname -m
capture proxy.txt scutil --proxy
capture dns.txt scutil --dns
capture routes.txt netstat -rn
capture default-route.txt route -n get default
capture services.txt networksetup -listallnetworkservices
capture vpn.txt scutil --nc list
capture interfaces.txt ifconfig

{
  printf 'captured_at_utc=%s\n' "$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  printf 'purpose=MAC-01 read-only baseline\n'
  printf 'network_changed_by_script=false\n'
  printf 'review_before_sharing=IP addresses, DNS servers, proxy details, VPN names may be present\n'
} > "$root/manifest.txt"

(cd "$root" && shasum -a 256 *.txt > SHA256SUMS)
printf 'Saved read-only snapshot to %s\n' "$root"
