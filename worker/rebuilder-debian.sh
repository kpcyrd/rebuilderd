#!/bin/sh
set -xe
# debrebuild.py needs to be run from the repo
/debrebuild/debrebuild.py --output="${REBUILDERD_OUTDIR}" --builder=mmdebstrap -- "${1}"
