---
name: Bug report
about: Report incorrect behaviour in the API
title: "[bug] "
labels: bug
---

**What happened**
A clear description of the bug.

**Endpoint + request**
e.g. `POST /v1/forms/fill` with `Content-Type: application/json`.
Do NOT attach confidential PDFs — describe the structure or attach a redacted/synthetic sample.

**Expected vs actual**

**Versions**
- API version (`GET /version` → `version`):
- Engine version (`GET /version` → `pdf_oxide_version`):
- Image tag / how you run it (docker run, compose, k8s):

**Logs / error envelope**
Paste the `application/problem+json` body (`error.code`, `error.request_id`).
