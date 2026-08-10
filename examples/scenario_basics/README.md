# Executable Scenario Basics

This package is the runnable 0.24 companion to the executable-package-test
documentation. It keeps a positive entry and an exact registered runtime error
under `tests/`, then executes each scenario with both evidence backends:

```bash
cd examples/scenario_basics
cellc test --frozen --offline --backend all --json
```

The simulator reports `development-non-consensus`; CKB-VM reports
`authoritative-runtime`. Neither result is chain, deployment, or transaction
syscall evidence.

The package also demonstrates the four-file verified-artifact bundle:

```bash
cellc build --frozen --offline
cellc verify-artifact build/main.elf --verify-sources --json
```

`--frozen` consumes the tracked dependency graph without adding local build
identity to `Cell.lock`. To exercise package identity verification, run an
ordinary locked build first:

```bash
cellc build --locked
cellc package verify --json
```
