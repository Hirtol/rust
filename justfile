shebang := if os() == 'windows' {
  'powershell.exe'
} else {
  '/usr/bin/env pwsh'
}

# Set shell for non-Windows OSs:
set shell := ["powershell", "-c"]

build:
    ./x build library --keep-stage 1
build-fresh:
    ./x build library

check:
    ./x check

run-with-macro:
    cargo +stage1 rustc -- -Z incremental_macro_expansion

run-with-profiler:
    cargo rustc -p rp_http --lib -- -Z self-profile -Z self-profile-events=default,args

crox-trace FILE:
    crox --minimum-duration 50 {{FILE}}
