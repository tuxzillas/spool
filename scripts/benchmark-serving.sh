#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: $0 SOURCE_FILE DOWNLOAD_URL" >&2
    exit 2
fi

source_file=$1
download_url=$2
output_file=$(mktemp "${TMPDIR:-/tmp}/spool-download.XXXXXX")
trap 'rm -f "$output_file"' EXIT HUP INT TERM

size=$(wc -c < "$source_file" | tr -d ' ')
source_hash=$(sha256sum "$source_file" | awk '{print $1}')

echo "source=$source_file"
echo "url=$download_url"
echo "bytes=$size"
echo "filesystem=$(df -T "$source_file" | awk 'NR==2 {print $2}')"
echo "storage=$(df -P "$source_file" | awk 'NR==2 {print $1}')"
echo "direct_read:"
direct_metrics=$(/usr/bin/time -f 'elapsed_seconds=%e cpu_user_seconds=%U cpu_system_seconds=%S' \
    sh -c 'dd if="$1" of=/dev/null bs=1M iflag=fullblock status=none' sh "$source_file" 2>&1)
echo "$direct_metrics"
direct_seconds=$(printf '%s\n' "$direct_metrics" | sed -n 's/^elapsed_seconds=//p')
awk -v bytes="$size" -v seconds="$direct_seconds" 'BEGIN { printf "throughput_mib_per_second=%.2f\n", bytes / seconds / 1048576 }'

echo "http_download:"
http_metrics=$(/usr/bin/time -f 'elapsed_seconds=%e cpu_user_seconds=%U cpu_system_seconds=%S' \
    curl --fail --location --silent --show-error --output "$output_file" "$download_url" 2>&1)
echo "$http_metrics"
http_seconds=$(printf '%s\n' "$http_metrics" | sed -n 's/^elapsed_seconds=//p')
awk -v bytes="$size" -v seconds="$http_seconds" 'BEGIN { printf "throughput_mib_per_second=%.2f\n", bytes / seconds / 1048576 }'

downloaded_hash=$(sha256sum "$output_file" | awk '{print $1}')
if [ "$source_hash" != "$downloaded_hash" ]; then
    echo "sha256_mismatch source=$source_hash downloaded=$downloaded_hash" >&2
    exit 1
fi
echo "sha256=$downloaded_hash"
