#!/bin/sh
set -xe
(cd common; cargo publish)
sleep 30 # wait for crates.io to do it's thing
(cd daemon && cargo publish)
(cd tools && cargo publish)
(cd worker && cargo publish)
git push origin master release --tags
