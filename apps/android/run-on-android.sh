#!/bin/sh

set -e
EXE="$1"
EXE_NAME=`basename $EXE`
adb push "$EXE" "/data/local/tmp/$EXE_NAME"
adb shell "chmod 755 /data/local/tmp/$EXE_NAME"
OUT="$(mktemp)"
MARK="ADB_SUCCESS!!!!!!!!!!!!!!"
adb shell "RUST_LOG=debug /data/local/tmp/$EXE_NAME && echo $MARK" 2>&1 | tee $OUT
grep $MARK $OUT