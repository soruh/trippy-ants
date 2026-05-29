#!/bin/bash

# Use the first argument as duration, or default to 30 if unset
DURATION=${1:-30}

# Initialize output file
echo "Threads,Median_SPS" > results.csv

echo "Building binary..."
cargo b --release

echo "Starting benchmark (${DURATION}s per test)..."

for threads in {1..64}; do
    # Print thread count without duration
    printf "Threads %-2d... " "$threads"

    # Run command with dynamic timeout
    last_line=$(RAYON_NUM_THREADS=$threads timeout "${DURATION}s" ./target/release/trippy-ants 2>&1 | tail -n 1)

    # Parse the median
    # We strip "MEDIAN" and whitespace to get just the number
    median=$(echo "$last_line" | awk -F '|' '{print $2}' | sed 's/MEDIAN//g' | xargs)

    # Handle failures if the process exits before outputting data
    if [ -z "$median" ]; then
        median="FAILED"
        echo "$median"
    else
        echo "${median} FPS"
    fi

    # Save to CSV
    echo "$threads,$median" >> results.csv
done

echo "Benchmark complete. Results saved to results.csv"

echo "Producing Plot..."
julia - <<EOF
using CSV, DataFrames, Plots
df = CSV.read("results.csv", DataFrame)
p = plot(df.Threads, df.Median_SPS, marker=:o, xlabel="Threads", ylabel="Median SPS", title="Performance vs Threads", legend=false)
savefig(p, "benchmark.svg")
EOF

echo "Plot saved to benchmark.svg"