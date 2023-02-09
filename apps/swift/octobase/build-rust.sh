#!/bin/bash

##################################################
# We call this from an Xcode run script.
##################################################

set -e

if [[ -z "$PROJECT_DIR" ]]; then
    echo "Must provide PROJECT_DIR environment variable set to the Xcode project directory." 1>&2
    exit 1
fi

cd $PROJECT_DIR

export PATH="$HOME/.cargo/bin:$PATH"

# Without this we can't compile on MacOS Big Sur
# https://github.com/TimNN/cargo-lipo/issues/41#issuecomment-774793892
if [[ -n "${DEVELOPER_SDK_DIR:-}" ]]; then
  export LIBRARY_PATH="${DEVELOPER_SDK_DIR}/MacOSX.sdk/usr/lib:${LIBRARY_PATH:-}"
fi

TARGETS=""
if [[ "$PLATFORM_NAME" = "iphonesimulator" ]]; then
    TARGETS="aarch64-apple-ios-sim,x86_64-apple-ios"
else
    TARGETS="aarch64-apple-ios,x86_64-apple-ios"
fi

# if [ $ENABLE_PREVIEWS == "NO" ]; then

  if [[ $CONFIGURATION == "Release" ]]; then
      echo "BUIlDING FOR RELEASE ($TARGETS)"

      cargo lipo --release --manifest-path ../../Cargo.toml  --targets $TARGETS
  else
      echo "BUIlDING FOR DEBUG ($TARGETS)"

      cargo lipo --manifest-path ../../Cargo.toml  --targets $TARGETS
  fi

# else
#   echo "Skipping the script because of preview mode"
# fi