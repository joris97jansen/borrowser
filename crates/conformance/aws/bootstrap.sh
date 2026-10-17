#!/bin/bash
set -euo pipefail
umask 077
export DEBIAN_FRONTEND=noninteractive

run_phase() {
  local phase="$1" seconds="$2" status
  # One bound covers the whole phase, including retries and service commands.
  if timeout --kill-after=5 "$seconds" /bin/bash -euo pipefail -c "$phase"; then
    return 0
  else
    status=$?
  fi
  # Preserve timeout/command status even if console reporting itself fails.
  printf '\nBORROWSER_AG4_BOOTSTRAP_FAILED @@RUN_ID@@ %s %d\n' "$phase" "$status" >/dev/console || true
  exit "$status"
}

setup() {
# Supplementary stop only. CloudFormation deletion is still required for cleanup.
systemd-run --unit=borrowser-ag4-lifetime --on-active=@@GUEST_STOP_MINUTES@@m /usr/sbin/poweroff
# Keep package resolution on the reviewed snapshot throughout this disposable boot.
systemctl mask --now apt-daily.service apt-daily-upgrade.service apt-daily.timer apt-daily-upgrade.timer unattended-upgrades.service
cat >/etc/apt/sources.list.d/ubuntu.sources <<'SOURCES'
Types: deb
URIs: https://snapshot.ubuntu.com/ubuntu/@@UBUNTU_SNAPSHOT@@/
Suites: noble noble-updates noble-security
Components: main universe
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
SOURCES
rm -f /etc/apt/sources.list
}
apt_update() {
apt-get -o Acquire::Retries=2 -o Acquire::https::Timeout=20 update
}
apt_install() {
apt-get -o Acquire::Retries=2 -o Acquire::https::Timeout=20 install -y --no-install-recommends \
  curl unzip apparmor python3 iproute2 gzip libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 \
  libatspi2.0-0t64 libcairo2 libcups2t64 libdbus-1-3 libexpat1 libgbm1 \
  libglib2.0-0t64 libnspr4 libnss3 libpango-1.0-0 libudev1 libx11-6 libxcb1 \
  libxcomposite1 libxdamage1 libxext6 libxfixes3 libxkbcommon0 libxrandr2
}
chromium_download() {
mkdir -p /opt/borrowser-ag2
curl --fail --location --proto '=https' --proto-redir '=https' --connect-timeout 15 \
  --max-time 180 --retry 2 --max-filesize 268435456 -o /tmp/chromium.zip '@@CHROME_URL@@'
}
archive() {
printf '%s  /tmp/chromium.zip\n' '@@CHROME_SHA256@@' | sha256sum --check --strict
unzip -q /tmp/chromium.zip -d /opt/borrowser-ag2
# The archive stays root-owned, but the non-root capture account must traverse
# its directories and read/execute Chromium despite the private bootstrap umask.
chmod -R a+rX /opt/borrowser-ag2
rm /tmp/chromium.zip
}
sandbox() {
cat >/etc/apparmor.d/borrowser-ag2-chromium <<'PROFILE'
abi <abi/4.0>,
include <tunables/global>
profile borrowser-ag2-chromium /opt/borrowser-ag2/chrome-linux64/chrome flags=(unconfined) {
  userns,
}
PROFILE
apparmor_parser -r /etc/apparmor.d/borrowser-ag2-chromium
grep -F 'borrowser-ag2-chromium ' /sys/kernel/security/apparmor/profiles
}
ssh_prepare() {
useradd --create-home --shell /bin/bash ag4
install -d -m 755 /opt/borrowser-ag4
printf '%s' '@@CAPTURE_WRAPPER_GZIP_B64@@' | base64 --decode | gzip --decompress >/opt/borrowser-ag4/guest_capture.py
chmod 644 /opt/borrowser-ag4/guest_capture.py
install -d -m 700 -o ag4 -g ag4 /home/ag4/.ssh /home/ag4/run /tmp/ag4
}
ssh_config() {
printf '%s' '@@SSH_KEY_B64@@' | base64 --decode >/home/ag4/.ssh/authorized_keys
chown ag4:ag4 /home/ag4/.ssh/authorized_keys
chmod 600 /home/ag4/.ssh/authorized_keys
cat >/etc/ssh/sshd_config.d/00-borrowser-ag4.conf <<'SSH'
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
AllowUsers ag4
AllowAgentForwarding no
AllowTcpForwarding no
X11Forwarding no
SSH
}
ssh_validate() {
# Standalone validation precedes ssh.service's RuntimeDirectory=sshd setup.
# Never repair or replace an existing, unreviewed runtime path.
if [[ ! -e /run/sshd && ! -L /run/sshd ]]; then
  install -d -m 0755 -o root -g root /run/sshd
fi
[[ -d /run/sshd && ! -L /run/sshd ]] || return 1
local metadata entry
metadata=$(stat -c '%u:%g:%a' /run/sshd)
[[ "$metadata" == '0:0:755' ]] || return 1
entry=$(find /run/sshd -mindepth 1 -maxdepth 1 -printf x -quit)
[[ -z "$entry" ]] || return 1
sshd -t
}
ssh_restart() {
systemctl restart ssh
}
ssh_identification() {
# A successful service transition alone does not prove a listening SSH endpoint.
# Receive only an identification line: no client banner, keys or authentication.
/usr/bin/python3 -I - "$1" <<'PY'
import socket
import sys
import time

deadline = time.monotonic() + 4
try:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as connection:
        connection.settimeout(max(0.001, deadline - time.monotonic()))
        connection.connect((sys.argv[1], 22))
        banner = bytearray()
        while len(banner) < 255:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError()
            connection.settimeout(remaining)
            byte = connection.recv(1)
            if not byte:
                break
            banner.extend(byte)
            if byte == b"\n":
                break
        if (not banner.endswith(b"\r\n") or not banner.startswith(b"SSH-2.0-")
                or not banner[8:-2].split(b" ", 1)[0]
                or any(byte < 32 or byte > 126 for byte in banner[:-2])):
            raise ValueError()
except (OSError, ValueError):
    raise SystemExit("SSH local identification unavailable or invalid")
PY
}
ssh_probe_loopback() {
ssh_identification 127.0.0.1
}
ssh_probe_private() {
# Local kernel observations only. No DNS, route-get to an external destination,
# IMDS or network request. Truncated/ambiguous observations cannot select a host.
local routes addresses address
routes=$(ip -j -4 route show default | head -c 8193)
addresses=$(ip -j -4 address show scope global | head -c 8193)
address=$(/usr/bin/python3 -I - "$routes" "$addresses" <<'PY'
import ipaddress
import json
import re
import sys

try:
    if any(len(value.encode()) > 8192 for value in sys.argv[1:]):
        raise ValueError()
    routes, links = (json.loads(value) for value in sys.argv[1:])
    if not isinstance(routes, list) or len(routes) != 1 or not isinstance(links, list):
        raise ValueError()
    route = routes[0]
    device = route["dev"]
    if (route["dst"] != "default" or route.get("type", "unicast") != "unicast"
            or "nexthops" in route or "multipath" in route
            or not isinstance(device, str) or not re.fullmatch(r"[a-zA-Z0-9_.-]{1,15}", device)
            or device == "lo"):
        raise ValueError()
    primary = [link for link in links if link["ifname"] == device]
    if len(primary) != 1:
        raise ValueError()
    link = primary[0]
    if (type(link["ifindex"]) is not int or link["ifindex"] <= 0
            or not isinstance(link["flags"], list) or "UP" not in link["flags"]
            or not isinstance(link["addr_info"], list) or len(link["addr_info"]) != 1):
        raise ValueError()
    info = link["addr_info"][0]
    address = ipaddress.IPv4Address(info["local"])
    if (info["family"] != "inet" or info["scope"] != "global"
            or info.get("secondary", False) is not False
            or type(info["prefixlen"]) is not int or not 1 <= info["prefixlen"] <= 32
            or str(address) != info["local"]
            or not any(address in ipaddress.IPv4Network(network)
                       for network in ("10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"))):
        raise ValueError()
except (ValueError, TypeError, KeyError):
    raise SystemExit("SSH primary private IPv4 unavailable or ambiguous")
print(address)
PY
)
ssh_identification "$address"
}
ready() {
local public_key
public_key=$(cut -d ' ' -f 1,2 /etc/ssh/ssh_host_ed25519_key.pub)
printf 'BORROWSER_AG4_READY @@RUN_ID@@ %s\n' "$public_key" >/dev/console
}

export -f setup apt_update apt_install chromium_download archive sandbox ssh_prepare ssh_config ssh_validate ssh_restart ssh_identification ssh_probe_loopback ssh_probe_private ready
# 920s phase sum + 13 * 5s kill allowances = 985s, below the controller's
# 1080s readiness window by 95s. See the operational contract and tests.
run_phase setup 30
run_phase apt_update 180
run_phase apt_install 420
run_phase chromium_download 180
run_phase archive 45
run_phase sandbox 15
run_phase ssh_prepare 10
run_phase ssh_config 5
run_phase ssh_validate 5
run_phase ssh_restart 10
run_phase ssh_probe_loopback 5
run_phase ssh_probe_private 10
run_phase ready 5
