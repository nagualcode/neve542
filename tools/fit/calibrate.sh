#!/usr/bin/env bash
# Full tape-path calibration against the hardware AP plots: alternate the
# non-linear fit (THD, all Saturation positions + Sat MAX response) and the
# replay-contour refit (model in the loop), writing results into tape.rs.
set -e
cd "$(dirname "$0")/../.."
build() { (cd snow653 && cargo build --release --example probe 2>&1 | grep -E "^error" -A5 || true); }
build
for i in 1 2; do
  python3 tools/fit/refit_contour.py | grep iter1
  python3 tools/fit/apply_fit.py; build
  python3 tools/fit/fit_thd.py | tail -13
  python3 tools/fit/apply_fit.py; build
done
python3 tools/fit/refit_contour.py | grep iter1
python3 tools/fit/apply_fit.py; build
