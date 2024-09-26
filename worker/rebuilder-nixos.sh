#!/bin/sh

set -x

echo "Rebuilding $1"
export PATH=/root/.nix-profile/bin:$PATH
whoami

DRV=$(cat $1 | grep StorePath | cut -d ":" -f 2)

REALIZED=$(nix-build --check $DRV)

nix-store --dump $REALIZED > $REBUILDERD_OUTDIR/out.nar
xz $REBUILDERD_OUTDIR/out.nar
HASH=$(nix-hash --base32 --type sha256 --flat $REBUILDERD_OUTDIR/out.nar.xz)
mv $REBUILDERD_OUTDIR/out.nar.xz $REBUILDERD_OUTDIR/$HASH.nar.xz
