#!/bin/bash

# Usage: ./bench_rayon.sh [duration] [max_threads]
DURATION=${1:-30}
MAX_THREADS=${2:-64}

echo "Threads,Mean,Median,StdDev" > results.csv

echo "Building binary..."
cargo b --release

echo "Starting benchmark (${DURATION}s per test, max threads: ${MAX_THREADS})..."

for threads in $(seq 1 "$MAX_THREADS"); do
    printf "Threads %-2d... " "$threads"

    raw_output=$(RAYON_NUM_THREADS=$threads timeout "${DURATION}s" ./target/release/trippy-ants 2>&1 | grep "Mean:")
    
    stats_line=$(echo "$raw_output" | tail -n 1)
    
    mean=$(echo "$stats_line" | awk -F '|' '{print $1}' | sed 's/Mean://g' | xargs)
    median=$(echo "$stats_line" | awk -F '|' '{print $2}' | sed 's/Median://g' | xargs)
    stddev=$(echo "$stats_line" | awk -F '|' '{print $3}' | sed 's/StdDev://g' | xargs)

    if [ -z "$median" ]; then
        echo "FAILED"
    else
        echo "Mean: $mean, Median: $median, StdDev: $stddev"
        echo "$threads,$mean,$median,$stddev" >> results.csv
    fi
done

echo "Benchmark complete. Results saved to results.csv"

echo "Producing Plot..."
julia - <<EOF
using CSV, DataFrames, Plots

df = CSV.read("results.csv", DataFrame)

# Conversion constant: 1e6 μs = 1 second
const C = 1e6

# Convert Time (ns) to SPS (Steps Per Second)
df.SPS_Mean = C ./ df.Mean
df.SPS_Median = C ./ df.Median

# Propagate error for StdDev: sigma_SPS = sigma_T * (C / Mean_T^2)
df.SPS_StdDev = df.StdDev .* (C ./ (df.Mean .^ 2))

# Plotting
p = plot(df.Threads, df.SPS_Mean, 
    ribbon = df.SPS_StdDev, 
    label="Mean SPS ± StdDev", 
    xlabel="Threads", 
    ylabel="SPS", 
    title="Performance vs Threads (Higher is Better)",
    fillalpha=0.2)

scatter!(df.Threads, df.SPS_Median, 
    label="Median SPS", 
    marker=:o, 
    color=:red)

savefig(p, "benchmark.svg")
EOF

echo "Plot saved to benchmark.svg"