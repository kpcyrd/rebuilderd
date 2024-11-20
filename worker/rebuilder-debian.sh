#!/bin/sh
set -xe
cd "$(dirname "$1")"
# for production it's useful to call debrebuild with --cache="$directory"
debrebuild --buildresult="${REBUILDERD_OUTDIR}" --builder=sbuild+unshare -- "${1}"
