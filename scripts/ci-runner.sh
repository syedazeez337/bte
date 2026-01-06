#!/usr/bin/env bash
set -e

BTE_VERSION="${BTE_VERSION:-latest}"

install_bte() {
    if [ "$BTE_VERSION" = "latest" ]; then
        cargo install bte --locked
    else
        cargo install bte --version "$BTE_VERSION" --locked
    fi
}

run_tests() {
    local scenario="${1:-examples/ratatui/counter.yaml}"
    echo "Running test: $scenario"
    bte run "$scenario"
}

generate_report() {
    local output="${1:-bte-report.json}"
    echo "Generating report: $output"
    bte report --format json --output "$output"
}

main() {
    install_bte
    run_tests
    generate_report
}

main "$@"
