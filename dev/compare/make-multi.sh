#!/usr/bin/env bash
# A clip with three audio tracks and three subtitle tracks (one forced), for the Detail scenarios: dev/compare/make-multi.sh [dir].
set -eu
dir=${1:-${MEDIA_DIR:-/tmp/streamline-player}}
work=$(mktemp -d)
printf '1\n00:00:01,000 --> 00:00:04,000\nHello there\n\n' >"$work/en.srt"
printf '1\n00:00:01,000 --> 00:00:04,000\nBonjour\n\n' >"$work/fr.srt"
printf '1\n00:00:01,000 --> 00:00:04,000\nForeign line\n\n' >"$work/enf.srt"
ffmpeg -y -v error -f lavfi -i testsrc=d=12:s=640x360:r=24 -f lavfi -i sine=f=440:d=12 -f lavfi -i sine=f=660:d=12 \
  -f lavfi -i sine=f=880:d=12 -i "$work/en.srt" -i "$work/fr.srt" -i "$work/enf.srt" \
  -map 0 -map 1 -map 2 -map 3 -map 4 -map 5 -map 6 -c:v libx265 -pix_fmt yuv420p -preset ultrafast -x265-params log-level=error -c:a aac -c:s srt \
  -metadata:s:a:0 language=eng -metadata:s:a:1 language=jpn -metadata:s:a:2 language=fre \
  -metadata:s:s:0 language=eng -metadata:s:s:1 language=fre -metadata:s:s:2 language=eng -disposition:s:2 forced \
  "$dir/multi.mkv"
rm -rf "$work"
