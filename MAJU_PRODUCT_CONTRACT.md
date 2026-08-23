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
- Whole-community deletion is an explicit relay-operator action, not a routine
  desktop or mobile control. The operator must freeze an inventory of the
  community's data and separately approve that exact inventory before deletion
  may begin.
- Deletion stops new community writes, drains work already in flight, removes
  community-owned data from every backing store, and leaves a permanent
  tombstone so the same community cannot be silently reused or resurrected.
  An operator may restore the community only while the request is still in its
  reversible approved or fenced stage; once object deletion begins there is no
  undo path.

## Agent identity and execution

- Agent definitions belong to one community. They never leak into another
  community merely because the same account or desktop app connects to both.
- A definition available to a community member is the shared blueprint; it is
  not another member's running agent. Each account creates or recovers its own
  agent identity from that blueprint. Different accounts never share an agent
  identity, credentials, or runner state.
- For one community, definition, and owning account there is at most one stable
  agent identity. Another device signed in as that account recovers the same
  identity instead of creating a duplicate.
- Definitions and teams never synchronize across community boundaries.
  Account-owned definition projections and agent identities synchronize only
  among that account's devices inside the community. Runtime credentials,
  local tools, process state, and execution eligibility remain specific to each
  device.
- Messages and project activity from every execution instance use the stable
  agent identity, so the agent keeps authority over its earlier work.
- The user turns an agent on wherever convenient. If the same agent is enabled
  on several online PCs, Maju automatically chooses exactly one representative
  runner and keeps the others ready as standby runners.
- Mentioning or attaching an agent starts it on the current device only when
  that account has no representative runner. If another device is already the
  representative, the existing runner handles the request and Maju must not
  create a new local standby as a side effect of the mention.
- Users do not choose a permanent primary PC. If the representative goes
  offline, one standby takes over automatically; two devices must not answer
  the same request as the same agent.

## Agent teams

- An agent definition belongs to at most one team. Team membership is part of
  the community definition, not a reason to mint another agent identity.
- Team instructions are layered onto every member's agent instructions on the
  next start or restart. Editing team instructions marks running members as
  needing a restart; deploying the team again is not required to apply them.
- **Deploy team to channel** attaches each account's existing team-member agent
  identities to that channel. Repeating it adds only missing members and never
  creates duplicate identities. Removing a definition from its team removes
  that team's instructions after the member next restarts.

## Agent harness compatibility

- Maju ships a first-party Antigravity ACP adapter with the desktop app. It
  uses the user's installed and signed-in `agy` CLI on Windows, macOS, or Linux;
  no third-party npm adapter is required. Models and model-specific reasoning
  levels come from live `agy models` output and a short local cache. Short
  prompts are passed directly without a shell, while long prompts travel
  through a temporary UTF-8 file instead of the OS command line. Only
  permission modes that map to real `agy` behavior are advertised.
- Maju advertises the stable ACP v1 initialization schema to external
  harnesses. It does not follow Buzz's experimental v2 advertisement while the
  request body remains v1-shaped, preserving existing Codex and Claude
  compatibility while allowing dual-version adapters to connect without a
  retry.
- Agent model and engine controls come from the ACP session's live
  `configOptions`, not adapter names or model-name parsing. `category=model`
  supplies the model picker and `category=thought_level` supplies reasoning;
  other advertised boolean or choice options live under advanced settings.
- Changing the model refreshes the full option set immediately, so dependent
  controls show every value the selected model actually advertises. Missing,
  stale, or invalid options fall back visibly to the engine default instead of
  being guessed or silently converted.
- ACP session option choices are stored as one complete, community-scoped
  definition value and synchronize among the owning account's devices. API
  keys, other secret environment values, executable paths, local tools, and
  running state remain device-local.
- A running agent keeps the ACP option values it started with. Editing those
  choices marks the agent as needing a restart; the new values take effect only
  after restart, without a second configured-versus-live value in the UI.

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
- Every project creation entry point uses the same full flow: a name, a
  repository access channel, and optional description, initial repository clone
  URL, and initial repository web URL. Creation publishes the project with its
  initial repository, and a project may contain multiple repositories.
