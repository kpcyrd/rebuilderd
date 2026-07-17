#!/bin/sh
# rebuilderd backend for OpenWrt apk packages.
#
# The actual work lives in the openwrt-rebuilder tool, which downloads +
# GPG-verifies the OpenWrt SDK, unpacks it and compiles the one package directly
# in this worker container (no nested container, no podman), like the
# archlinux/debian backends. openwrt-rebuilder is invoked with plain CLI
# parameters, so this wrapper derives the package identity from the inputs and
# passes it as flags.
#
# The apk carries neither arch nor release, so both come from the URL path
# (.../packages/<arch>/<feed>/<file>.apk under snapshots/ or releases/<rel>/);
# the package identity is the .apk filename.
#
# openwrt-rebuilder builds with an SDK, which is published per target/subtarget
# rather than per package arch, and the apk URL names only the arch. Resolving
# one to the other is the caller's job, so this wrapper picks a target for the
# arch below. Several ABI-compatible targets share an arch — any of them yields
# a usable SDK — so one representative per arch is enough.
#
# Inputs (from the worker):
#   $1                    downloaded original .apk (identity = its filename)
#   REBUILDERD_OUTDIR     directory to write the rebuilt .apk into
#   REBUILDERD_INPUT_URL  the .apk URL
#
# Optional env (mapped to openwrt-rebuilder flags): OPENWRT_UPSTREAM,
# OPENWRT_SDK_CACHE, OPENWRT_DL_DIR, OPENWRT_KEYRING_DIR. OPENWRT_TARGET
# overrides the SDK target picked for the package arch.

set -eu

: "${REBUILDERD_OUTDIR:?must be set}"
: "${REBUILDERD_INPUT_URL:?must be set — need the .apk URL to derive arch/release}"

# --- derive arch / release from the .apk URL ---
url="${REBUILDERD_INPUT_URL%%[?#]*}"
case "$url" in
    */packages/*) ;;
    *) echo "no /packages/ segment in REBUILDERD_INPUT_URL: $url" >&2; exit 1 ;;
esac
after="${url#*/packages/}"
ARCH="${after%%/*}"
case "$url" in
    */snapshots/packages/*) RELEASE="SNAPSHOT" ;;
    */releases/*/packages/*) tail="${url#*/releases/}"; RELEASE="${tail%%/*}" ;;
    *) echo "cannot derive release from REBUILDERD_INPUT_URL: $url" >&2; exit 1 ;;
esac
[ -n "$ARCH" ] || { echo "failed to parse arch from $url" >&2; exit 1; }

# --- pick an SDK target for the package arch ---
# One representative target per arch (see OPENWRT_TARGET to override). Arches
# are stable; if OpenWrt adds one, add it here.
case "$ARCH" in
    aarch64_cortex-a53)        TARGET="mediatek/filogic" ;;
    aarch64_cortex-a72)        TARGET="bcm27xx/bcm2711" ;;
    aarch64_cortex-a76)        TARGET="bcm27xx/bcm2712" ;;
    aarch64_generic)           TARGET="armsr/armv8" ;;
    arm_arm1176jzf-s_vfp)      TARGET="bcm27xx/bcm2708" ;;
    arm_arm926ej-s)            TARGET="at91/sam9x" ;;
    arm_cortex-a15_neon-vfpv4) TARGET="armsr/armv7" ;;
    arm_cortex-a5_vfpv4)       TARGET="at91/sama5" ;;
    arm_cortex-a7)             TARGET="mediatek/mt7629" ;;
    arm_cortex-a7_neon-vfpv4)  TARGET="bcm27xx/bcm2709" ;;
    arm_cortex-a7_vfpv4)       TARGET="at91/sama7" ;;
    arm_cortex-a8_vfpv3)       TARGET="omap/generic" ;;
    arm_cortex-a9)             TARGET="bcm53xx/generic" ;;
    arm_cortex-a9_neon)        TARGET="imx/cortexa9" ;;
    arm_cortex-a9_vfpv3-d16)   TARGET="mvebu/cortexa9" ;;
    arm_fa526)                 TARGET="gemini/generic" ;;
    arm_xscale)                TARGET="kirkwood/generic" ;;
    armeb_xscale)              TARGET="ixp4xx/generic" ;;
    i386_pentium-mmx)          TARGET="x86/legacy" ;;
    i386_pentium4)             TARGET="x86/generic" ;;
    loongarch64_generic)       TARGET="loongarch64/generic" ;;
    mips64_mips64r2)           TARGET="malta/be64" ;;
    mips64_octeonplus)         TARGET="octeon/generic" ;;
    mips64el_mips64r2)         TARGET="malta/le64" ;;
    mips_24kc)                 TARGET="ath79/generic" ;;
    mips_mips32)               TARGET="bmips/bcm6318" ;;
    mipsel_24kc)               TARGET="ramips/mt7621" ;;
    mipsel_24kc_24kf)          TARGET="pistachio/generic" ;;
    mipsel_74kc)               TARGET="bcm47xx/mips74k" ;;
    mipsel_mips32)             TARGET="bcm47xx/generic" ;;
    powerpc64_e5500)           TARGET="qoriq/generic" ;;
    powerpc_464fp)             TARGET="apm821xx/nand" ;;
    powerpc_8548)              TARGET="mpc85xx/p1010" ;;
    riscv64_generic)           TARGET="sifiveu/generic" ;;
    x86_64)                    TARGET="x86/64" ;;
    *) echo "no SDK target known for arch '$ARCH'; add it to $0 or set OPENWRT_TARGET" >&2; exit 1 ;;
esac
TARGET="${OPENWRT_TARGET:-$TARGET}"

echo "=== openwrt package rebuild ==="
echo "arch:    $ARCH  (sdk target: $TARGET)"
echo "release: $RELEASE"

set -- package \
    --package "$(basename -- "$1")" \
    --target "$TARGET" \
    --release "$RELEASE" \
    --output "$REBUILDERD_OUTDIR"

# Map the worker's optional cache/mirror env to flags (skip when unset).
[ -z "${OPENWRT_UPSTREAM:-}" ]    || set -- "$@" --upstream "$OPENWRT_UPSTREAM"
[ -z "${OPENWRT_SDK_CACHE:-}" ]   || set -- "$@" --sdk-cache "$OPENWRT_SDK_CACHE"
[ -z "${OPENWRT_DL_DIR:-}" ]      || set -- "$@" --dl-dir "$OPENWRT_DL_DIR"
[ -z "${OPENWRT_KEYRING_DIR:-}" ] || set -- "$@" --keyring-dir "$OPENWRT_KEYRING_DIR"

exec openwrt-rebuilder "$@"
