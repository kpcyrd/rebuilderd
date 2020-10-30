#!/bin/sh
set -xe
timeout 1d repro -- "${2}"
