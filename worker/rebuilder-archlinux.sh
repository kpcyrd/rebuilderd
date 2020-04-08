#!/usr/bin/env bash
set -xe
dir=$(mktemp -dt rebuild.XXXXXXXXXX)
trap 'rm -r "$dir"' EXIT
wget -P "${dir}" -- "${1}"
file=$(basename "${1}")
repro -- "${dir}/${file}"
