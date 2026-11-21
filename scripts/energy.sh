#!/bin/sh
# Samples the CPU, energy impact and memory of the app, the WebKit processes that draw its web UI, and Safari, on macOS.
# Energy impact is the number Activity Monitor shows and charges against the battery.
# Run it once for each setup you compare, doing the same thing in each, and quit every other WebKit app first,
# because macOS names a WKWebView's processes the same for every app.
#
# Usage: scripts/energy.sh [seconds] [label]
set -eu

seconds=${1:-60}
label=${2:-sample}
out="energy-${label}.txt"

if [ "$(uname)" != Darwin ]; then
    echo "This script reads macOS's top, so it runs on macOS only." >&2
    exit 1
fi

echo "Sampling for ${seconds} seconds as \"${label}\"..."
# One sample a second; top's first sample has no CPU or energy figures, so awk skips it.
top -l "$((seconds + 1))" -s 1 -stats pid,command,cpu,power,mem >"$out"

awk -v label="$label" '
    /^PID/ { sample++; next }
    sample < 2 || NF < 5 { next }
    {
        group = ""
        if ($2 ~ /^ricochet/) group = "app"
        else if ($2 ~ /^com\.apple\.WebKi/) group = "webkit"
        else if ($2 ~ /^Safari/) group = "safari"
        if (group == "") next
        cpu[group] += $3
        power[group] += $4
        mem = $5
        unit = substr(mem, length(mem))
        sub(/[KMG+-]+$/, "", mem)
        if (unit == "K") mem /= 1024
        if (unit == "G") mem *= 1024
        last[group, $1] = mem
        pids[group, $1] = 1
    }
    END {
        samples = sample - 1
        printf "%s, %d samples\n", label, samples
        printf "%-8s %10s %14s %12s\n", "group", "avg CPU %", "avg energy", "memory MB"
        split("app webkit safari", order, " ")
        for (i = 1; i <= 3; i++) {
            g = order[i]
            m = 0
            for (key in pids) {
                split(key, part, SUBSEP)
                if (part[1] == g) m += last[key]
            }
            if (cpu[g] == "" && m == 0) continue
            printf "%-8s %10.1f %14.1f %12.0f\n", g, cpu[g] / samples, power[g] / samples, m
            tc += cpu[g] / samples; tp += power[g] / samples; tm += m
        }
        printf "%-8s %10.1f %14.1f %12.0f\n", "total", tc, tp, tm
    }
' "$out"
echo "Raw samples are in ${out}."
