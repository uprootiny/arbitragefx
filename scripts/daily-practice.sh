#!/bin/bash
# Daily Practice - Survey, Test, Reflect
#
# Usage: ./scripts/daily-practice.sh [command]
#
# Commands:
#   morning   - Morning survey and setup
#   test      - Run hypothesis tests
#   reflect   - Evening reflection prompts
#   full      - Complete daily cycle

set -e
cd "$(dirname "$0")/.."

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

morning_survey() {
    echo -e "${BLUE}=== MORNING SURVEY ===${NC}"
    echo ""
    echo "Date: $(date '+%Y-%m-%d %A')"
    echo ""

    echo -e "${GREEN}Hypothesis Status:${NC}"
    ./target/release/research_lab hypotheses 2>/dev/null || echo "  (research_lab not built)"
    echo ""

    echo -e "${GREEN}Recommended Actions:${NC}"
    ./target/release/research_lab actions 2>/dev/null | head -15 || echo "  (run research_lab actions)"
    echo ""

    echo -e "${GREEN}System Health:${NC}"
    cargo test --lib 2>&1 | tail -1
    echo ""

    echo -e "${YELLOW}Reflection:${NC}"
    echo "  What hypothesis will I test today?"
    echo "  What would change my mind about my current beliefs?"
    echo "  Am I approaching this with right intention?"
    echo ""
}

run_tests() {
    echo -e "${BLUE}=== HYPOTHESIS TESTING ===${NC}"
    echo ""

    if [ -f "data/btc_1h_180d.csv" ]; then
        echo "Running all hypothesis tests..."
        ./target/release/research_lab test-all data/btc_1h_180d.csv
    else
        echo "No data file found. Download market data first."
    fi
    echo ""

    echo -e "${GREEN}Best by Regime:${NC}"
    ./target/release/research_lab best 2>/dev/null || echo "  (run tests first)"
    echo ""
}

evening_reflect() {
    echo -e "${BLUE}=== EVENING REFLECTION ===${NC}"
    echo ""
    echo "Date: $(date '+%Y-%m-%d')"
    echo ""

    echo -e "${YELLOW}Review:${NC}"
    echo "  1. What did I learn today about the markets?"
    echo "  2. Were my predictions well-calibrated?"
    echo "  3. Did I act with right intention?"
    echo "  4. What hypothesis should I update?"
    echo ""

    echo -e "${GREEN}Evidence Summary:${NC}"
    ./target/release/research_lab evidence 2>/dev/null | tail -10 || echo "  (no evidence yet)"
    echo ""

    echo -e "${YELLOW}Gratitude:${NC}"
    echo "  What worked well today?"
    echo "  What am I grateful for in this practice?"
    echo ""

    echo -e "${YELLOW}Tomorrow:${NC}"
    echo "  What is the most important thing to test tomorrow?"
    echo "  What would I do differently?"
    echo ""
}

full_cycle() {
    morning_survey
    echo "---"
    echo ""
    run_tests
    echo "---"
    echo ""
    evening_reflect
}

case "${1:-morning}" in
    morning|m)
        morning_survey
        ;;
    test|t)
        run_tests
        ;;
    reflect|r)
        evening_reflect
        ;;
    full|f)
        full_cycle
        ;;
    *)
        echo "Usage: $0 [morning|test|reflect|full]"
        exit 1
        ;;
esac
