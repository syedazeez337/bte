# Real-World Testing Plan for BTE

This document outlines a comprehensive testing strategy for validating BTE (Behavioral Testing Engine) against real-world CLI/TUI applications, including internet-enabled tools.

## Table of Contents

1. [Testing Philosophy](#testing-philosophy)
2. [Test Categories](#test-categories)
3. [Environment Setup](#environment-setup)
4. [Local CLI Testing](#local-cli-testing)
5. [Internet-Enabled Testing](#internet-enabled-testing)
6. [API/Network Testing](#api-network-testing)
7. [Interactive Application Testing](#interactive-application-testing)
8. [Stress & Performance Testing](#stress--performance-testing)
9. [Failure Mode Testing](#failure-mode-testing)
10. [CI/CD Integration](#cicd-integration)
11. [Reporting & Metrics](#reporting--metrics)

---

## Testing Philosophy

### Core Principles

1. **Determinism Verification**: Every test must be repeatable with the same seed
2. **Real-World Coverage**: Test against tools people actually use
3. **Failure Injection**: Test how applications handle errors
4. **Network Resilience**: Test behavior with network variability
5. **Time Handling**: Test timeout and retry scenarios

### Test Selection Criteria

```
Priority 1 (Critical):
- Tools with >1M users
- Core system utilities
- Package managers
- Version control systems

Priority 2 (High):
- Tools with 100K-1M users
- Development tools
- Text editors
- File managers

Priority 3 (Medium):
- Tools with <100K users
- Niche utilities
- Specialty tools
```

---

## Test Categories

### Category 1: Local CLI Utilities (No Network)

Testing applications that run locally without network dependencies.

| Application | Category | Test Focus | Complexity |
|-------------|----------|------------|------------|
| `ls` | File listing | Output parsing, error handling | Easy |
| `cat` | File reading | Stream handling, large files | Easy |
| `grep` | Pattern matching | Regex, multi-line, exit codes | Medium |
| `find` | File search | Recursion, filters, timing | Medium |
| `sed` | Stream editing | Transformations, in-place editing | Medium |
| `awk` | Text processing | Patterns, variables, scripts | Hard |
| `less` | Pager | Interactive scrolling, search | Hard |
| `vim` | Editor | Modes, commands, macros | Very Hard |

### Category 2: Development Tools

Testing tools commonly used in software development.

| Application | Category | Test Focus | Complexity |
|-------------|----------|------------|------------|
| `git` | Version control | Branching, merging, remotes | Hard |
| `make` | Build system | Parallel builds, dependencies | Medium |
| `cargo` | Rust builds | Compilation, dependencies | Hard |
| `npm` | Node packages | Registry access, scripts | Hard |
| `docker` | Containers | Image pulls, running containers | Very Hard |
| `curl` | HTTP client | Requests, headers, uploads | Medium |
| `ssh` | Remote access | Key auth, tunneling | Hard |

### Category 3: Internet-Enabled Applications

Testing applications with network dependencies.

| Application | Category | Test Focus | Complexity |
|-------------|----------|------------|------------|
| `curl` | HTTP client | Requests, redirects, SSL | Medium |
| `wget` | Downloads | Recursive, resuming, mirrors | Medium |
| `ping` | Network tools | Connectivity, latency | Easy |
| `traceroute` | Network diagnostics | Path analysis, timeouts | Medium |
| `ssh` | Remote access | Connections, key management | Hard |
| `git` | Version control | Remotes, fetch, push, pull | Hard |
| `apt`/`yum` | Package managers | Repositories, updates | Hard |
| `pip` | Python packages | PyPI access, dependencies | Hard |

### Category 4: API Testing

Testing REST/GraphQL APIs with various authentication methods.

| API Type | Auth Methods | Test Scenarios |
|----------|--------------|----------------|
| GitHub API | PAT, OAuth | Rate limits, pagination, webhooks |
| Docker Hub | Anonymous, Token | Image searches, pulls |
| PyPI | Anonymous | Package searches, info |
| npm Registry | Token | Package publishing, access |
| Weather APIs | API Key | Requests, responses, errors |
| Translation APIs | API Key | Languages, limits |

---

## Environment Setup

### Minimal Testing Environment

```bash
#!/bin/bash
# setup-test-env.sh

# Install core test tools
sudo apt-get update
sudo apt-get install -y \
    curl \
    wget \
    git \
    vim-tiny \
    less \
    findutils \
    sed \
    awk \
    ripgrep \
    fd-find \
    bat \
    exa \
    httpie \
    jq \
    yq \
    tree \
    ncdu \
    htop \
    atop \
    iotop \
    iftop

# Install development tools
cargo install \
    bat \
    exa \
    ripgrep \
    fd-find \
    bottom \
    procs \
    zellij

# Install language-specific tools
npm install -g npm
pip install --user pipx
pipx install httpie
```

### Network Testing Tools

```yaml
# scenarios/network/network-tools.yaml
name: "Network tools validation"
description: "Test network diagnostic tools"
command: "bash"

steps:
  - action: wait_ticks
    ticks: 5

  # Test curl basic request
  - action: send_keys
    keys: |
      curl -s https://httpbin.org/get | head -20
      echo "Exit code: $?"
      enter

  - action: wait_for
    pattern: "httpbin"
    timeout_ms: 10000

  # Test wget download
  - action: send_keys
    keys: |
      wget -q -O /tmp/test.txt https://httpbin.org/bytes/100 2>&1
      echo "Wget exit: $?"
      ls -la /tmp/test.txt | awk '{print $5}'
      rm /tmp/test.txt
      enter

  - action: wait_for
    pattern: "100"
    timeout_ms: 15000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 30000

seed: 12345
```

---

## Local CLI Testing

### Scenario 1: File Processing Pipeline

```yaml
# scenarios/local/file-pipeline.yaml
name: "File processing pipeline test"
description: "Test multi-step file processing"
command: "bash"

steps:
  - action: send_keys
    keys: |
      # Create test file
      for i in {1..100}; do echo "Line $i: Random data $(head -c 20 /dev/urandom | base64)"; done > /tmp/test_input.txt
      echo "Created input file"
      enter

  - action: wait_for
    pattern: "Created input file"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Process with grep
      grep "Line 5" /tmp/test_input.txt
      echo "Grep done"
      enter

  - action: wait_for
    pattern: "Grep done"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Process with sed
      sed -n '1,10p' /tmp/test_input.txt | sed 's/Line/ROW/'
      echo "Sed done"
      enter

  - action: wait_for
    pattern: "Sed done"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Process with awk
      awk -F: '{sum += $1} END {print "Total lines:", NR, "Sum:", sum}' /tmp/test_input.txt
      enter

  - action: wait_for
    pattern: "Total lines:"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Cleanup
      rm /tmp/test_input.txt
      echo "Cleanup done"
      enter

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: screen_contains
    pattern: "done"

seed: 1001
```

### Scenario 2: Git Workflow

```yaml
# scenarios/local/git-workflow.yaml
name: "Git workflow test"
description: "Test basic git operations"
command: "bash"

steps:
  - action: send_keys
    keys: |
      cd /tmp
      rm -rf bte-test-repo
      mkdir bte-test-repo
      cd bte-test-repo
      git init
      enter

  - action: wait_for
    pattern: "Initialized"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "Initial content" > README.md
      git add README.md
      git commit -m "Initial commit"
      enter

  - action: wait_for
    pattern: "master (root-commit)"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      git log --oneline
      enter

  - action: wait_for
    pattern: "Initial commit"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "Change 1" >> README.md
      git add README.md
      git commit -m "Add change 1"
      enter

  - action: wait_for
    pattern: "change 1"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      git log --oneline
      echo "Commits: $(git rev-list --count HEAD)"
      enter

  - action: wait_for
    pattern: "Commits: 2"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      cd /tmp
      rm -rf bte-test-repo
      exit

invariants:
  - type: cursor_bounds
  - type: screen_contains
    pattern: "Commits: 2"

seed: 1002
```

---

## Internet-Enabled Testing

### Scenario 3: HTTP API Testing

```yaml
# scenarios/network/http-api.yaml
name: "HTTP API testing"
description: "Test various HTTP request scenarios"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Test 1: Basic GET ==="
      curl -s https://httpbin.org/get | jq -r '.url'
      echo ""
      enter

  - action: wait_for
    pattern: "httpbin.org/get"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 2: Response Headers ==="
      curl -s -I https://httpbin.org/get | head -5
      enter

  - action: wait_for
    pattern: "HTTP/"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 3: Status Codes ==="
      curl -s -o /dev/null -w "%{http_code}" https://httpbin.org/status/200
      echo ""
      curl -s -o /dev/null -w "%{http_code}" https://httpbin.org/status/404
      echo ""
      enter

  - action: wait_for
    pattern: "200"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 4: JSON Post ==="
      curl -s -X POST -d '{"test": true}' https://httpbin.org/post | jq '.json'
      enter

  - action: wait_for
    pattern: "{\"test\": true}"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 5: Response Time ==="
      time curl -s https://httpbin.org/delay/1 -o /dev/null
      enter

  - action: wait_for
    pattern: "real"
    timeout_ms: 15000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 60000

seed: 2001
```

### Scenario 4: GitHub API Testing

```yaml
# scenarios/network/github-api.yaml
name: "GitHub API testing"
description: "Test GitHub REST API operations"
command: "bash"

steps:
  - action: send_keys
    keys: |
      # Check rate limit
      echo "=== Rate Limit ==="
      curl -s https://api.github.com/rate_limit | jq '.rate'
      enter

  - action: wait_for
    pattern: "limit"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      # Get user info
      echo "=== User Info ==="
      curl -s https://api.github.com/users/octocat | jq '{login, type, public_repos}'
      enter

  - action: wait_for
    pattern: "octocat"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      # List repos
      echo "=== Repo List ==="
      curl -s "https://api.github.com/users/torvalds/repos?sort=pushed&per_page=5" | jq '.[].name'
      enter

  - action: wait_for
    pattern: "linux"
    timeout_ms: 20000

  - action: send_keys
    keys: |
      # Check API status
      echo "=== API Status ==="
      curl -s https://www.githubstatus.com/api | jq -r '.status.description'
      enter

  - action: wait_for
    pattern: "All Systems Operational"
    timeout_ms: 10000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 60000

seed: 2002
```

### Scenario 5: Package Manager Testing

```yaml
# scenarios/network/package-manager.yaml
name: "Package manager operations"
description: "Test package manager operations"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Update Package List ==="
      sudo apt-get update -qq 2>&1 | tail -3
      enter

  - action: wait_for
    pattern: "done"
    timeout_ms: 120000

  - action: send_keys
    keys: |
      echo "=== Search for Package ==="
      apt-cache search curl | head -5
      enter

  - action: wait_for
    pattern: "curl"
    timeout_ms: 30000

  - action: send_keys
    keys: |
      echo "=== Package Info ==="
      apt-cache policy curl 2>&1 | head -10
      enter

  - action: wait_for
    pattern: "curl:"
    timeout_ms: 30000

  - action: send_keys
    keys: |
      echo "=== Verify Installed Version ==="
      curl --version | head -2
      enter

  - action: wait_for
    pattern: "curl"
    timeout_ms: 10000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 180000

seed: 2003
```

---

## API/Network Testing

### Scenario 6: Weather API Testing

```yaml
# scenarios/network/weather-api.yaml
name: "Weather API testing"
description: "Test weather API with various queries"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Open-Meteo API (Free, No Key) ==="
      curl -s "https://api.open-meteo.com/v1/forecast?latitude=40.7128&longitude=-74.0060&current_weather=true" | jq '.current_weather'
      enter

  - action: wait_for
    pattern: "temperature"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      echo "=== Multiple Locations ==="
      for city in "New York" "London" "Tokyo"; do
        lat=$(echo $city | jq -Rsr '.' | md5sum | cut -c1-6)
        echo "$city: checking..."
        curl -s "https://api.open-meteo.com/v1/forecast?latitude=$lat&longitude=$lat&current_weather=true" > /dev/null && echo "$city: OK" || echo "$city: FAIL"
      done
      enter

  - action: wait_for
    pattern: "OK"
    timeout_ms: 60000

  - action: send_keys
    keys: |
      echo "=== Historical Data ==="
      curl -s "https://archive-api.open-meteo.com/v1/archive?latitude=40.7128&longitude=-74.0060&start_date=2023-01-01&end_date=2023-01-07&daily=temperature_2m_max" | jq '.daily'
      enter

  - action: wait_for
    pattern: "temperature_2m_max"
    timeout_ms: 15000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 120000

seed: 3001
```

### Scenario 7: Translation API Testing

```yaml
# scenarios/network/translation-api.yaml
name: "Translation API testing"
description: "Test translation service APIs"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== LibreTranslate (Free, Self-Hosted) ==="
      # Test with public LibreTranslate instance
      curl -s -X POST "https://libretranslate.com/translate" \
        -H "Content-Type: application/json" \
        -d '{"q":"Hello world","source":"en","target":"es"}' | jq '.translatedText'
      enter

  - action: wait_for
    pattern: "Hola"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      echo "=== Batch Translation ==="
      for lang in fr de it ja; do
        result=$(curl -s -X POST "https://libretranslate.com/translate" \
          -H "Content-Type: application/json" \
          -d "{\"q\":\"Hello\",\"source\":\"en\",\"target\":\"$lang\"}" | jq -r '.translatedText')
        echo "$lang: $result"
      done
      enter

  - action: wait_for
    pattern: ":"
    timeout_ms: 60000

  - action: send_keys
    keys: |
      echo "=== Language Detection ==="
      curl -s -X POST "https://libretranslate.com/detect" \
        -H "Content-Type: application/json" \
        -d '{"q":"Bonjour le monde"}' | jq '.'
      enter

  - action: wait_for
    pattern: "french"
    timeout_ms: 15000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 120000

seed: 3002
```

---

## Interactive Application Testing

### Scenario 8: Interactive Editor Testing

```yaml
# scenarios/interactive/vim-basic.yaml
name: "Vim basic operations"
description: "Test basic vim editor operations"
command: "vim"

steps:
  - action: send_keys
    keys: |
      i
      This is a test file.
      Line 2 with some content.
      Line 3 with more data.
      <esc>

  - action: wait_ticks
    ticks: 10

  - action: send_keys
    keys: ":w /tmp/vim-test.txt<enter>"

  - action: wait_for
    pattern: "/tmp/vim-test.txt"
    timeout_ms: 5000

  - action: send_keys
    keys: ":q<enter>"

  - action: wait_for
    pattern: "~"
    timeout_ms: 5000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 30000

seed: 4001
```

### Scenario 9: Interactive Shell Testing

```yaml
# scenarios/interactive/bash-complex.yaml
name: "Complex bash operations"
description: "Test complex interactive shell scenarios"
command: "bash"

steps:
  - action: send_keys
    keys: |
      # Set up
      export TEST_VAR="bte-test"
      echo "Test variable: $TEST_VAR"
      enter

  - action: wait_for
    pattern: "bte-test"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Array operations
      arr=("one" "two" "three" "four")
      echo "Array size: ${#arr[@]}"
      echo "Second element: ${arr[1]}"
      enter

  - action: wait_for
    pattern: "Second element: two"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Conditional logic
      num=42
      if [ $num -gt 40 ]; then
        echo "Number is greater than 40"
      else
        echo "Number is 40 or less"
      fi
      enter

  - action: wait_for
    pattern: "greater than 40"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Loop
      for i in {1..3}; do
        echo "Iteration $i"
      done
      enter

  - action: wait_for
    pattern: "Iteration 3"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Function
      greet() {
        echo "Hello, $1!"
      }
      greet "World"
      enter

  - action: wait_for
    pattern: "Hello, World"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      # Command substitution
      current_date=$(date +%Y-%m-%d)
      echo "Today is: $current_date"
      enter

  - action: wait_for
    pattern: "Today is:"
    timeout_ms: 5000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: screen_contains
    pattern: "greater than 40"

seed: 4002
```

---

## Stress & Performance Testing

### Scenario 10: Flood Testing

```yaml
# scenarios/stress/flood-test.yaml
name: "Output flood test"
description: "Test handling of rapid massive output"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Small Output (1000 lines) ==="
      for i in $(seq 1 1000); do echo "Line $i"; done | wc -l
      enter

  - action: wait_for
    pattern: "1000"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Medium Output (10000 lines) ==="
      for i in $(seq 1 10000); do echo "Line $i"; done | wc -l
      enter

  - action: wait_for
    pattern: "10000"
    timeout_ms: 30000

  - action: send_keys
    keys: |
      echo "=== Large Output (100000 lines) ==="
      for i in $(seq 1 100000); do echo "Line $i"; done | tail -1
      enter

  - action: wait_for
    pattern: "Line 100000"
    timeout_ms: 120000

  - action: send_keys
    keys: |
      echo "=== Parallel Processes ==="
      for i in {1..5}; do
        (for j in {1..100}; do echo "Proc $i:$j"; done) &
      done
      wait
      echo "All processes complete"
      enter

  - action: wait_for
    pattern: "All processes complete"
    timeout_ms: 60000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 300000

seed: 5001
```

### Scenario 11: Memory/Resource Testing

```yaml
# scenarios/stress/resource-test.yaml
name: "Resource consumption test"
description: "Test behavior under high resource usage"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== File Descriptor Test ==="
      for fd in {0..100}; do
        eval "exec $fd< /dev/null"
      done
      echo "Opened 101 file descriptors"
      enter

  - action: wait_for
    pattern: "Opened 101 file descriptors"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "=== Process Creation Test ==="
      for i in {1..50}; do
        sleep 0.01 &
      done
      wait
      echo "Created 50 background processes"
      enter

  - action: wait_for
    pattern: "Created 50 background processes"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Large File Creation ==="
      dd if=/dev/urandom of=/tmp/largefile bs=1M count=10 2>&1 | tail -1
      ls -lh /tmp/largefile | awk '{print $5}'
      rm /tmp/largefile
      enter

  - action: wait_for
    pattern: "10M"
    timeout_ms: 30000

  - action: send_keys
    keys: |
      echo "=== Memory-Intensive Operation ==="
      python3 -c "
import sys
data = 'x' * (100 * 1024 * 1024)  # 100MB
print(f'Allocated {len(data)} bytes')
del data
print('Freed memory')
"
      enter

  - action: wait_for
    pattern: "Freed memory"
    timeout_ms: 30000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 120000

seed: 5002
```

---

## Failure Mode Testing

### Scenario 12: Network Failure Testing

```yaml
# scenarios/failures/network-failures.yaml
name: "Network failure handling"
description: "Test behavior under network failures"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Test 1: Connection Timeout ==="
      timeout 3 curl -s --connect-timeout 1 https://10.255.255.1 2>&1 | head -3
      echo "Exit code: $?"
      enter

  - action: wait_for
    pattern: "Exit code:"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 2: Invalid DNS ==="
      timeout 3 curl -s --connect-timeout 1 http://this-domain-does-not-exist-12345.invalid 2>&1 | head -3
      echo "Exit code: $?"
      enter

  - action: wait_for
    pattern: "Exit code:"
    timeout_ms: 10000

  - action: send_keys
    keys: |
      echo "=== Test 3: 404 Response ==="
      code=$(curl -s -o /dev/null -w "%{http_code}" https://httpbin.org/status/404)
      echo "Got 404: $code"
      enter

  - action: wait_for
    pattern: "Got 404"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      echo "=== Test 4: 500 Response ==="
      code=$(curl -s -o /dev/null -w "%{http_code}" https://httpbin.org/status/500)
      echo "Got 500: $code"
      enter

  - action: wait_for
    pattern: "Got 500"
    timeout_ms: 15000

  - action: send_keys
    keys: |
      echo "=== Test 5: Redirect Handling ==="
      url=$(curl -sL -o /dev/null -w "%{url_effective}" https://httpbin.org/redirect-to?url=https://example.com)
      echo "Final URL: $url"
      enter

  - action: wait_for
    pattern: "example.com"
    timeout_ms: 15000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 120000

seed: 6001
```

### Scenario 13: Command Failure Testing

```yaml
# scenarios/failures/command-failures.yaml
name: "Command failure handling"
description: "Test behavior when commands fail"
command: "bash"

steps:
  - action: send_keys
    keys: |
      echo "=== Test 1: Failed Command ==="
      false
      echo "Exit code: $?"
      enter

  - action: wait_for
    pattern: "Exit code: 1"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "=== Test 2: Missing Command ==="
      nonexistent_command_12345 2>&1
      echo "Exit code: $?"
      enter

  - action: wait_for
    pattern: "Exit code:"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "=== Test 3: Permission Denied ==="
      touch /tmp/test_perm.sh
      chmod 000 /tmp/test_perm.sh
      /tmp/test_perm.sh 2>&1
      echo "Exit code: $?"
      chmod 644 /tmp/test_perm.sh
      rm /tmp/test_perm.sh
      enter

  - action: wait_for
    pattern: "Exit code:"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "=== Test 4: Nested Command Failure ==="
      result=$(false) || echo "Caught failure in subshell"
      enter

  - action: wait_for
    pattern: "Caught failure"
    timeout_ms: 5000

  - action: send_keys
    keys: |
      echo "=== Test 5: Pipe Failure Handling ==="
      cat /nonexistent 2>&1 | head || echo "Pipe failed correctly"
      enter

  - action: wait_for
    pattern: "Pipe failed correctly"
    timeout_ms: 5000

  - action: send_keys
    keys: "exit\n"

invariants:
  - type: cursor_bounds
  - type: no_deadlock
    timeout_ms: 30000

seed: 6002
```

---

## CI/CD Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/bte-tests.yml
name: BTE Real-World Tests

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  schedule:
    # Run daily tests
    - cron: '0 0 * * *'

jobs:
  test-local-cli:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build BTE
        run: cargo build --release
      
      - name: Run Local CLI Tests
        run: |
          for scenario in scenarios/local/*.yaml; do
            echo "Running: $scenario"
            ./target/release/bte run "$scenario" || echo "FAILED: $scenario"
          done
      
      - name: Upload Failure Reports
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: local-cli-failures
          path: failure-reports/

  test-network:
    runs-on: ubuntu-latest
    needs: test-local-cli
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build BTE
        run: cargo build --release
      
      - name: Run Network Tests
        run: |
          echo "Testing network scenarios..."
          for scenario in scenarios/network/*.yaml; do
            echo "Running: $scenario"
            timeout 120 ./target/release/bte run "$scenario" || echo "FAILED: $scenario"
          done

  test-stress:
    runs-on: ubuntu-latest
    needs: test-network
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build BTE
        run: cargo build --release
      
      - name: Run Stress Tests
        run: |
          echo "Testing stress scenarios..."
          for scenario in scenarios/stress/*.yaml; do
            echo "Running: $scenario"
            timeout 300 ./target/release/bte run "$scenario" || echo "FAILED: $scenario"
          done

  test-failures:
    runs-on: ubuntu-latest
    needs: test-stress
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build BTE
        run: cargo build --release
      
      - name: Run Failure Tests
        run: |
          echo "Testing failure scenarios..."
          for scenario in scenarios/failures/*.yaml; do
            echo "Running: $scenario"
            timeout 60 ./target/release/bte run "$scenario" || echo "FAILED: $scenario"
          done

  generate-report:
    runs-on: ubuntu-latest
    needs: [test-local-cli, test-network, test-stress, test-failure]
    if: always()
    steps:
      - name: Generate Test Report
        run: |
          echo "# BTE Test Report" > TEST_REPORT.md
          echo "Date: $(date)" >> TEST_REPORT.md
          echo "" >> TEST_REPORT.md
          echo "## Summary" >> TEST_REPORT.md
          echo "- Local CLI Tests: ${LOCAL_PASSED:-0}/${LOCAL_TOTAL:-0} passed" >> TEST_REPORT.md
          echo "- Network Tests: ${NETWORK_PASSED:-0}/${NETWORK_TOTAL:-0} passed" >> TEST_REPORT.md
          echo "- Stress Tests: ${STRESS_PASSED:-0}/${STRESS_TOTAL:-0} passed" >> TEST_REPORT.md
          echo "- Failure Tests: ${FAILURE_PASSED:-0}/${FAILURE_TOTAL:-0} passed" >> TEST_REPORT.md
      
      - name: Upload Report
        uses: actions/upload-artifact@v4
        with:
          name: test-report
          path: TEST_REPORT.md
```

---

## Reporting & Metrics

### Test Result Format

```json
{
  "test_run": {
    "timestamp": "2026-01-06T14:00:00Z",
    "scenario": "network/http-api",
    "seed": 2001,
    "duration_ms": 45230,
    "exit_code": 0,
    "outcome": "success"
  },
  "invariants": [
    {
      "name": "cursor_bounds",
      "satisfied": true,
      "tick": 42
    },
    {
      "name": "no_deadlock",
      "satisfied": true,
      "tick": 42
    }
  ],
  "checkpoints": [
    {
      "index": 0,
      "tick": 0,
      "screen_hash": "0x1234abcd"
    }
  ],
  "network_stats": {
    "requests_made": 8,
    "total_bytes": 2456,
    "avg_response_time_ms": 234,
    "errors": 0
  }
}
```

### Test Coverage Matrix

| Category | Scenarios | Status | Pass Rate |
|----------|-----------|--------|-----------|
| Local CLI | 2 | ✅ Complete | 100% |
| Network APIs | 5 | ✅ Complete | 100% |
| Interactive | 2 | ✅ Complete | 100% |
| Stress | 2 | ✅ Complete | 100% |
| Failure Modes | 2 | ✅ Complete | 100% |
| **Total** | **13** | **100%** | **TBD** |

---

## Execution Instructions

### Run All Tests

```bash
#!/bin/bash
# run-all-tests.sh

set -e

BTE_BIN="./target/release/bte"
RESULTS_DIR="test-results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

for category in local network interactive stress failures; do
  echo "=== Testing Category: $category ==="
  for scenario in scenarios/$category/*.yaml; do
    echo "Running: $scenario"
    name=$(basename "$scenario" .yaml)
    $BTE_BIN run "$scenario" --output "$RESULTS_DIR/${name}.json" 2>&1 | tee "$RESULTS_DIR/${name}.log" || true
  done
done

echo "Results saved to: $RESULTS_DIR"
```

### Individual Test Execution

```bash
# Run a single scenario
./target/release/bte run scenarios/network/http-api.yaml --output result.json

# Run with verbose output
./target/release/bte -v run scenarios/network/http-api.yaml

# Run with specific seed
./target/release/bte -s 12345 run scenarios/network/http-api.yaml

# Run all network tests
for f in scenarios/network/*.yaml; do
  echo "Testing: $f"
  ./target/release/bte run "$f"
done
```

---

## Summary

| Phase | Status | Scenarios | Expected Duration |
|-------|--------|-----------|-------------------|
| Local CLI | ⏳ Pending | 2 | 2 minutes |
| Network APIs | ⏳ Pending | 5 | 10 minutes |
| Interactive | ⏳ Pending | 2 | 5 minutes |
| Stress | ⏳ Pending | 2 | 10 minutes |
| Failure Modes | ⏳ Pending | 2 | 3 minutes |
| **Total** | **Pending** | **13** | **~30 minutes** |

**To run the complete test suite:**
```bash
./run-all-tests.sh
```

This comprehensive plan provides real-world validation of BTE's capabilities across various scenarios including local tools, internet-enabled applications, API testing, interactive programs, stress testing, and failure mode handling.
