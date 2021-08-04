#!/bin/sh
set -xe
debrebuild --buildresults="${REBUILDERD_OUTDIR}" --builder=mmdebstrap -- "${1}"
