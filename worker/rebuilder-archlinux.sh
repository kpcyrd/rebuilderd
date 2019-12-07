#!/bin/sh
set -xe
N=2
wget -O /dev/null -- "$1"
for x in `seq $N`; do
    echo "pretending to build $x/$N ..."
    sleep 10
done
exit 1
