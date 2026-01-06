#!/bin/bash
# run-realworld-tests.sh
# Real-World Testing Runner for BTE

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

BTE_BIN="${BTE_BIN:-./target/release/bte}"
RESULTS_DIR="test-results/$(date +%Y%m%d-%H%M%S)"
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIPPED=0

# Create results directory
mkdir -p "$RESULTS_DIR"

echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║         BTE Real-World Test Runner                        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "BTE Binary: $BTE_BIN"
echo "Results Directory: $RESULTS_DIR"
echo "Started: $(date)"
echo ""

run_test() {
    local scenario="$1"
    local category="$2"
    local name=$(basename "$scenario" .yaml)
    
    echo -e "${YELLOW}[RUN]${NC} $category/$name"
    
    # Check if BTE binary exists
    if [ ! -f "$BTE_BIN" ]; then
        echo -e "${RED}[ERROR]${NC} BTE binary not found: $BTE_BIN"
        TOTAL_SKIPPED=$((TOTAL_SKIPPED + 1))
        return 1
    fi
    
    # Run the test with timeout
    local timeout_seconds=300
    local output_file="$RESULTS_DIR/${category}-${name}.json"
    local log_file="$RESULTS_DIR/${category}-${name}.log"
    
    if timeout "$timeout_seconds" "$BTE_BIN" run "$scenario" --output "$output_file" > "$log_file" 2>&1; then
        local exit_code=0
        echo -e "${GREEN}[PASS]${NC} $category/$name"
        TOTAL_PASS=$((TOTAL_PASS + 1))
    else
        local exit_code=$?
        if [ $exit_code -eq 124 ]; then
            echo -e "${RED}[TIMEOUT]${NC} $category/$name (>$timeout_seconds s)"
        else
            echo -e "${RED}[FAIL]${NC} $category/$name (exit: $exit_code)"
        fi
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
    fi
    
    return $exit_code
}

# Test categories in order
declare -a CATEGORIES=("local" "network" "interactive" "stress" "failures")

for category in "${CATEGORIES[@]}"; do
    echo ""
    echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}Category: $category${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
    
    # Check if category exists
    if [ ! -d "scenarios/$category" ]; then
        echo -e "${YELLOW}[SKIP]${NC} No scenarios found for category: $category"
        continue
    fi
    
    # Count scenarios
    scenario_count=$(ls -1 scenarios/$category/*.yaml 2>/dev/null | wc -l)
    if [ "$scenario_count" -eq 0 ]; then
        echo -e "${YELLOW}[SKIP]${NC} No .yaml files in scenarios/$category"
        continue
    fi
    
    echo "Found $scenario_count scenario(s)"
    echo ""
    
    # Run each scenario
    for scenario in scenarios/$category/*.yaml; do
        run_test "$scenario" "$category" || true
    done
done

echo ""
echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Test Summary                            ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "Total Passed:   ${GREEN}$TOTAL_PASS${NC}"
echo -e "Total Failed:   ${RED}$TOTAL_FAIL${NC}"
echo -e "Total Skipped:  ${YELLOW}$TOTAL_SKIPPED${NC}"
echo -e "Total Tests:    $((TOTAL_PASS + TOTAL_FAIL + TOTAL_SKIPPED))"
echo ""
echo -e "Success Rate:   ${GREEN}$(echo "scale=1; $TOTAL_PASS * 100 / ($TOTAL_PASS + $TOTAL_FAIL)" | bc 2>/dev/null || echo "N/A")%${NC}"
echo ""
echo "Results saved to: $RESULTS_DIR"
echo "Completed: $(date)"

# Generate summary report
cat > "$RESULTS_DIR/SUMMARY.md" << EOF
# BTE Real-World Test Summary

**Date:** $(date)
**BTE Binary:** $BTE_BIN

## Results

| Category | Passed | Failed | Skipped | Total |
|----------|--------|--------|---------|-------|
EOF

for category in "${CATEGORIES[@]}"; do
    local passed=$(grep -c "\[PASS\]" "$RESULTS_DIR"/*$category*.log 2>/dev/null || echo 0)
    local failed=$(grep -c "\[FAIL\]" "$RESULTS_DIR"/*$category*.log 2>/dev/null || echo 0)
    local skipped=$(grep -c "\[SKIP\]" "$RESULTS_DIR"/*$category*.log 2>/dev/null || echo 0)
    local total=$((passed + failed + skipped))
    echo "| $category | $passed | $failed | $skipped | $total |" >> "$RESULTS_DIR/SUMMARY.md"
done

cat >> "$RESULTS_DIR/SUMMARY.md" << EOF

## Overall

- **Total Passed:** $TOTAL_PASS
- **Total Failed:** $TOTAL_FAIL  
- **Total Skipped:** $TOTAL_SKIPPED

## Failed Tests

EOF

for log in "$RESULTS_DIR"/*.log; do
    if grep -q "\[FAIL\]\|\[TIMEOUT\]" "$log" 2>/dev/null; then
        echo "### $(basename "$log" .log)" >> "$RESULTS_DIR/SUMMARY.md"
        grep -A2 "\[FAIL\]\|\[TIMEOUT\]" "$log" >> "$RESULTS_DIR/SUMMARY.md" 2>/dev/null || true
        echo "" >> "$RESULTS_DIR/SUMMARY.md"
    fi
done

echo ""
echo "Summary report: $RESULTS_DIR/SUMMARY.md"

# Exit with appropriate code
if [ $TOTAL_FAIL -gt 0 ]; then
    exit 1
else
    exit 0
fi
