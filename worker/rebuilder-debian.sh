#!/bin/sh

set -e

cat <<-EOF
===============================================================================

About this build: this rebuild was performed using the methodology developed by
the Debian Reproducible Builds effort, with the goal of reproducing Debian
binary packages distributed via ftp.debian.org. The rebuild uses the same build
dependency package versions as the original build, as recorded in the
corresponding .buildinfo file from buildinfos.debian.net.

For more information please go to https://reproduce.debian.net or join
#debian-reproducible on irc.debian.org

===============================================================================

EOF
CACHE="$PWD/debian-cache"
echo "Preparing download of sources for $1"
SOURCE=$(basename "$1" | cut -d '_' -f1)
# take VERSION from .buildinfo file (which has the epoch) but drop +bX suffix from binNMUs
VERSION=$(grep ^Version: "$1" | cut -d ' ' -f2- | sed -r -e 's#\+b[[:digit:]]+$##g')
echo "Source: $SOURCE"
echo "Version: $VERSION"
echo "rebuilderd-worker node: $(hostname)"
echo
echo "+------------------------------------------------------------------------------+"
echo "| Downloading sources                          $(date -u -R) |"
echo "+------------------------------------------------------------------------------+"
echo

cd "$(dirname "$1")"
mkdir -p etc/apt
mkdir -p var/lib/apt/lists/
# Use disk-backed temporary directory, otherwise `mktemp` would attempt to but everything in RAM
export TMPDIR=/var/tmp
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie main non-free-firmware' > etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-trixie-security-automatic.gpg] https://deb.debian.org/debian-security trixie-security main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie-updates main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie-proposed-updates main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian trixie-backports main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian forky main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian sid main non-free-firmware' >> etc/apt/sources.list
echo 'deb-src [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] https://deb.debian.org/debian experimental main non-free-firmware' >> etc/apt/sources.list
apt-get -o Dir=. update -q
apt-get -o Dir=. source -qq --print-uris "$SOURCE=$VERSION"
apt-get -o Dir=. source -qq --download-only "$SOURCE=$VERSION"
dcmd sha256sum *.dsc

echo
echo "+------------------------------------------------------------------------------+"
echo "| Calling debrebuild                           $(date -u -R) |"
echo "+------------------------------------------------------------------------------+"
echo
echo "Rebuilding $SOURCE=$VERSION in $(pwd) now."
set -x
nice debrebuild --buildresult="${REBUILDERD_OUTDIR}" --builder=sbuild+unshare --cache="${CACHE}" -- "${1}"
set +x
echo
echo "+------------------------------------------------------------------------------+"
echo "| Finished running debrebuild                  $(date -u -R) |"
echo "+------------------------------------------------------------------------------+"
