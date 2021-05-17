#!/bin/sh
set -xe
debrebuild --output="${REBUILDERD_OUTDIR}" --builder=mmdebstrap -- "${1}"
