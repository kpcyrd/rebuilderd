#!/bin/sh
set -xe
wget -O /dev/null -- "$1"
for x in 1 2 3 4; do
    echo "pretending to build $x/4 ..."
    sleep 10
done
exit 1
