#!/bin/sh
set -eux
IMG_PATH="$1"
TAG=$(basename "$IMG_PATH" | sed -nr 's/tails-amd64-([0-9a-z~\.]+)\.[^\]+/\1/p' | sed 's/~/-/g')
REPO_URL='https://gitlab.tails.boum.org/tails/tails.git'

export TAILS_BUILD_OPTIONS="nomergebasebranch forcecleanup"

# cleanup possible leftovers
virsh vol-list default | awk '{print $1}' | grep ^tails-builder- | xargs -rL1 virsh vol-delete --pool default

# setup temporary directory
WORK_DIR=$(mktemp -d -t tails.XXXXXX)
trap '{ rm -rf -- "$WORK_DIR"; }' EXIT
# set the folder public so libvirt user can access it
chmod 0711 "$WORK_DIR"

# import gpg keys to authenticate source code
export HOME="$WORK_DIR/home"
mkdir -m 0700 -- "$HOME"
# Fetch the latest key over https
curl -sSf https://tails.boum.org/tails-signing.key | gpg --import

# clone repo
REPO_DEST="$WORK_DIR/tails"
# doesn't work even with nomergebasebranch
#git clone --depth=1 --branch "$TAG" -- "$REPO_URL" "$REPO_DEST"
git clone --branch "$TAG" -- "$REPO_URL" "$REPO_DEST"
cd "$REPO_DEST"
git verify-tag -v -- "$TAG"
git submodule update --init

# read and export SOURCE_DATE_EPOCH to normalize the build time
SOURCE_DATE_EPOCH=$(date --utc --date="$(dpkg-parsechangelog --show-field=Date)" '+%s')
export SOURCE_DATE_EPOCH

# build the image
# libvirtd needs to be started, /var/run/libvirt needs to be mounted if run inside a container
ARTIFACTS="$REBUILDERD_OUTDIR" rake build

# list build outputs
ls -la "$REBUILDERD_OUTDIR"
