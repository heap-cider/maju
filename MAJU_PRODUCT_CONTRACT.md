# Maju Product Contract

This document contains the current product decisions that distinguish Maju
from Buzz or deliberately constrain how Maju follows Buzz. It is a product
contract, not proof that every decision has already been implemented.

Keep only the decisions that are valid now. Replace or remove stale decisions
instead of preserving history. Do not add implementation details, file or API
inventories, work logs, release notes, speculative ideas, or undecided plans.

## Product identity

- The user-facing product name is **Maju**. Buzz is named only when explaining
  the read-only upstream project or comparing an upstream release.
- User-facing builds, packages, links, and deployment instructions use Maju
  names and Maju-owned distribution locations.

## Supported releases

- Maju's release targets are the Windows desktop app, Android app, and Linux
  relay.
- macOS and iOS are not Maju release targets.
- Public releases are distributed through the Maju GitHub repository. Relay
  containers are distributed through Maju's public GHCR package.
- The Android app checks Maju's latest public GitHub release in-app. It can
  download the signed APK and hand it to Android's installer, where the user
  gives the final update approval. Updating in place preserves app data and
  desktop pairing; deleting the installed app is not part of the update flow.

## Self-hosted communities and onboarding

- Maju connects to relays operated by the user or someone they trust. It does
  not offer a hosted Maju or Block SaaS community in onboarding.
- First-run community setup asks for one relay address or invite link and uses
  the identity already created or imported on the device. It does not ask the
  user to choose a hosting provider, community type, or community role.
- A relay URL is the community boundary. Additional communities may be added
  later, but each is another explicitly configured self-hosted relay.

## Agent identity and execution

- An agent has one stable identity within its community. Moving or starting
  that agent on another device must not create a new author identity.
- Agent definitions and identity are synchronized for the same owning account;
  runtime credentials, local tools, and running state remain specific to each
  execution device.
- Messages and project activity from every execution instance use the stable
  agent identity, so the agent keeps authority over its earlier work.
- The user turns an agent on wherever convenient. If the same agent is enabled
  on several online PCs, Maju automatically chooses exactly one representative
  runner and keeps the others ready as standby runners.
- Users do not choose a permanent primary PC. If the representative goes
  offline, one standby takes over automatically; two devices must not answer
  the same request as the same agent.

## Agent harness compatibility

- Maju advertises the stable ACP v1 initialization schema to external
  harnesses. It does not follow Buzz's experimental v2 advertisement while the
  request body remains v1-shaped, preserving existing Codex and Claude
  compatibility while allowing dual-version adapters to connect without a
  retry.

## Account devices

- The account private key is the account identity and may sign in on multiple
  PCs. A device id records an installation and execution location; it is not a
  second user identity.
- Settings includes **내 기기**, showing current and previously signed-in PCs,
  online state, last seen time, Maju version, and which agents are representative
  or standby on each PC.
- The Agents view shows the actual representative PC and whether this PC is
  standing by. A stale local process error must not make an agent look offline
  while another PC is actively running it.
- Disconnecting another device ends that official Maju login session and stops
  its agent runners. It does not revoke the account private key, so the user may
  explicitly sign in again with that key.

## Community authority

- Community owners, and admins to whom they delegate moderation, may edit or
  remove channel messages, forum posts, and forum replies regardless of whether
  a person or an agent authored them.
- Moderation never rewrites or impersonates the original event signature.
  Owner edits and removals remain explicit, attributable management actions.

## Projects

- Projects belong to the active community and are published to its relay.
- When a community has no projects, the Projects page shows one primary create
  action that opens the real project creation flow directly.
- Creating a project starts with its name and optional description. Maju
  derives the repository identifier and creates the relay repository; links to
  an existing remote repository or website are optional advanced settings.
