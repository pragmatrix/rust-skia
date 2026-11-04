#!/usr/bin/env bash
# Builds and runs `app` for every combination of link mode, keep-alive reference, and profile, and
# checks that a missing export fails the link. Exits with 1 if any check fails.
cd "$(dirname "$0")"
exe=""
[ "${OS:-}" = Windows_NT ] && exe=.exe
failures=0

rustc -vV
echo "--- Does rustc pass symbols.o to the link?"
cargo build -q -p app >/dev/null 2>&1
touch app/src/main.rs
cargo rustc -q -p app -- --print link-args 2>/dev/null | grep -o '[^ "]*symbols\.o' | head -1 \
    || echo "no symbols.o in the link arguments"

echo "--- Link and run"
for bundle in 1 0; do
  for keep in no yes; do
    features=""
    [ $keep = yes ] && features="--features keep"
    for profile in dev release release-thin release-fat; do
      label="bundle=$bundle keep=$keep profile=$profile"
      dir=$profile
      [ $profile = dev ] && dir=debug
      if BUNDLE=$bundle cargo build -q -p app --profile $profile $features >build.log 2>&1; then
        output=$("${CARGO_TARGET_DIR:-target}/$dir/app$exe" 2>run.err)
        status=$?
        if [ "$output" = "core=7 backend=42" ]; then
          echo "ok      $label"
        else
          echo "WRONG   $label: output '$output', exit status $status"
          cat run.err
          failures=$((failures + 1))
        fi
      else
        echo "FAILED  $label"
        tail -40 build.log
        failures=$((failures + 1))
      fi
    done
  done
done

echo "--- A missing export fails the link"
if cargo build -q -p app --features omit-get-size >build.log 2>&1; then
  echo "WRONG   linked without wgpuBufferGetSize"
  failures=$((failures + 1))
elif grep -q wgpuBufferGetSize build.log; then
  echo "ok      the link failed and named wgpuBufferGetSize"
else
  echo "WRONG   the link failed for another reason"
  tail -40 build.log
  failures=$((failures + 1))
fi

echo "--- $failures failure(s)"
[ $failures = 0 ]
