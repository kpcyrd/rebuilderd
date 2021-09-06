#!/bin/sh
set -xe
debrebuild --buildresult="${REBUILDERD_OUTDIR}" --builder=mmdebstrap -- "${1}"
