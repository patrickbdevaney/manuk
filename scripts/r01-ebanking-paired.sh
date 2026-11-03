#!/bin/bash
# Paired probe: openssl / manuk a11y-dump / Chrome dump-dom against one host, interleaved.
# usage: r01-ebanking-paired.sh N OUT.tsv
N=${1:-20}; OUT=${2:-/dev/stdout}; H=ebanking.bdm-sa.com
CH=$(google-chrome --version 2>/dev/null | awk '{print $NF}')
printf 'round\tutc\topenssl_cert\tmanuk_rc\tmanuk_secs\tchrome_rc\tchrome_bytes\tchrome_ver\n' > "$OUT"
for i in $(seq 1 $N); do
  touch .git/sprint-working
  u=$(date -u +%FT%TZ)
  oc=$(echo | timeout 10 openssl s_client -connect $H:443 -servername $H 2>/dev/null | grep -c 'BEGIN CERTIFICATE')
  [ "$oc" -gt 0 ] && oc=1
  s=$(date +%s.%N); timeout 60 target/release/a11y-dump https://$H/ >/dev/null 2>&1; mr=$?; e=$(date +%s.%N)
  ms=$(echo "$e - $s" | bc | xargs printf '%.1f')
  cb=$(timeout 60 google-chrome --headless=new --no-sandbox --dump-dom https://$H/ 2>/dev/null | wc -c); cr=$([ "$cb" -gt 1000 ] && echo 0 || echo 1)
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' $i $u $oc $mr $ms $cr $cb $CH >> "$OUT"
done
