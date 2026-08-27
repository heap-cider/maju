# Maju Product Contract

This file is an allowlist of the current ways Maju must differ from Buzz.
Behavior not listed here follows the latest synchronized Buzz release.

Keep each owner decision to the smallest user-visible outcome, normally one
short bullet of one or two sentences. Do not add implementation details, code
paths, history, inferred effects, or edge-case policy unless the owner chose
those details too. Derived engineering requirements belong in code, tests, or
engineering documentation.

## Fork invariants

### Product identity and distribution

- The user-facing product name is **Maju**. Builds, packages, links, and release
  instructions use Maju names and Maju-owned distribution locations.
- Maju releases Windows desktop, Android, and Linux relay artifacts through the
  Maju GitHub repository and GHCR. It does not release macOS or iOS artifacts.

### Self-hosted communities

- Maju connects to relays operated by the user or someone they trust. It does
  not offer a hosted Maju or Block SaaS community in onboarding.
- First-run setup asks for one relay address or invite link. Each configured
  relay URL is a separate community boundary.

## Intentional product deltas

### Agent identity and execution

- Within one community, the same owner's same agent definition uses one stable
  agent identity across devices.
- If that agent is enabled on several online devices, exactly one device is the
  representative runner. Another device must not start a duplicate while that
  representative is available.

### Agent harness compatibility

- Maju keeps its first-party Antigravity adapter, stable ACP v1 compatibility,
  and agent controls supplied by live ACP `configOptions`.

### Projects

- When there are no projects, the Projects screen still offers a way to start
  creating one.
