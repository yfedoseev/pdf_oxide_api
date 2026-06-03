## What & why

<!-- Brief description. Link the issue (e.g. Closes #12). -->

## Checklist

- [ ] Tests added/updated (TDD — a regression guard for this change)
- [ ] `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` green
- [ ] No PDF content / field values / filenames logged or echoed in errors
- [ ] Every `pdf_oxide` call goes through `state.cpu.run(...)`
- [ ] `CHANGELOG.md` updated if user-facing
- [ ] OpenAPI (`openapi.yaml` / `/openapi.json`) updated if the contract changed
