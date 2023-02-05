#!/bin/sh
set -xe
NOCHECK=1 archlinux-repro -o "${REBUILDERD_OUTDIR}" -- "${1}"
