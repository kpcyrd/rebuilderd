#!/bin/sh

set -xe

rpmfile="${1}"
# extract nvr
nvr=$(rpm -qp --queryformat '%{SOURCERPM}' ${rpmfile} | sed s'/.src.rpm$//')

fedora-repro-build "${nvr}"
