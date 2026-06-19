# Blog Brief

## Spec

- `agentpm/specs/2026-06-09-private-namespaces-early-pricing/spec.md`

## Is this blog-worthy?

Yes.

This spec is blog-worthy because it marks the point where AgentPM stops being only a public registry and becomes a credible private registry for individuals and teams.

That is a real product shift:

- not just publish packages publicly
- not just install from the open ecosystem
- create namespaces deliberately
- separate user and org ownership models
- support private package access
- add the first real pricing and billing surfaces around private usage

## What shipped

This spec added the private-registry foundation across the AgentPM stack:

- Registry/API
  - namespaces now support:
    - `kind: user | org`
    - `visibility: public | private`
  - users can have multiple user namespaces
  - org namespaces support members and roles:
    - owner
    - admin
    - member
  - shared authorization helpers now enforce:
    - namespace visibility
    - publish permissions
    - member-management permissions
    - billing-management permissions
  - private package and namespace reads are now access-controlled across:
    - namespace detail
    - namespace package lists
    - namespace activity
    - tool detail
    - agent detail
    - template detail
    - versions
    - readme
    - security
  - search is now auth-aware for private results
  - install and publish flows now enforce private access and entitlement state
  - entitlement state now supports:
    - free
    - trialing
    - pro
    - team
    - past due
    - canceled
    - manual grants

- Website
  - `/pricing` now exists with:
    - Free
    - Pro
    - Team
    - comparison table
  - `/profile` now supports:
    - multiple namespaces
    - user vs org namespace creation
    - public vs private namespace selection
    - namespace settings
    - org member management
    - billing section
  - namespace detail pages now reflect:
    - private access rules
    - management entry points by role
    - member lists for org namespaces
  - private search and detail flows now work correctly for authorized users

- CLI
  - private install and publish flows now respect namespace access and entitlement checks
  - exact-version and `agent.lock` flows do not bypass private access controls
  - docs and behavior now align better around explicit namespace selection when a user can publish to more than one namespace

- Billing foundation
  - Lemon Squeezy checkout, webhook ingestion, and billing-state reconciliation are implemented
  - billing/profile UI exists

## Why this matters

The broader AgentPM point this work proves is:

- packaging ecosystems do not stay useful for long if they only work in public
- teams need a boundary between:
  - what they share publicly
  - what they keep internal

Without private namespaces, AgentPM is mostly a public discovery and publishing surface.

With this work, AgentPM can now support a more serious internal-registry story:

- internal tools
- internal agents
- internal templates
- team ownership
- role-based access
- secure install and publish behavior

That is a much stronger product position than “a public package manager for agent-related artifacts.”

## Strongest concepts and angles

### Angle 1: Public package managers are not enough for real teams

Core idea:

- a useful packaging layer needs both open distribution and private internal distribution

Why it is interesting:

- most teams want to reuse internal automation long before they want to publish it publicly
- private registries are not an edge feature; they are the point where packaging becomes operationally useful

Possible framing:

- “A package manager becomes much more interesting the moment teams can use it privately, not just publicly.”

### Angle 2: Private registries need one consistent authorization model

Core idea:

- private access is not one check in one route
- it has to hold across:
  - search
  - detail pages
  - installs
  - publishes
  - member management

Why it is interesting:

- this is where many systems get leaky or inconsistent
- the work here was not just adding a visibility flag; it was threading that flag through every meaningful surface

Possible framing:

- “A private package is only private if search, detail pages, installs, and publishes all agree.”

### Angle 3: The right first billing story is simple and bounded

Core idea:

- Free stays free for the public ecosystem
- Pro covers private solo usage
- Team covers shared private org usage

Why it is interesting:

- it validates paid private usage without overbuilding enterprise billing
- it is a disciplined first pricing model, not a giant monetization matrix

Possible framing:

- “The first paid layer for AgentPM is not complicated: public stays free, private solo is Pro, private team usage is Team.”

### Angle 4: A private registry is not only a backend feature

Core idea:

- a real private-registry launch touches:
  - authorization
  - CLI install/publish
  - search
  - namespace management
  - pricing
  - billing
  - docs

Why it is interesting:

- it shows that “private packages” are not one isolated feature
- the product only feels coherent when the registry, web app, CLI, and pricing model all tell the same story

Possible framing:

- “A private registry is not one toggle. It is a coordinated product surface across API, CLI, web, and billing.”

## What was learned

- Namespace kind and visibility needed to become first-class early.
  - once user vs org and public vs private are real, many downstream behaviors become much easier to reason about

- Access control had to be treated as a system-wide concern.
  - route-by-route fixes were not enough
  - search, package reads, install flows, publish flows, and namespace pages all needed to align

- Org roles needed to stay intentionally small.
  - owner, admin, and member were enough for this phase
  - adding more role complexity early would have slowed the product down without adding much clarity

- Billing integration was as much about state reconciliation as checkout.
  - webhook shape
  - idempotency
  - subscription state mapping
  all mattered more than just “redirect to checkout”

## Tie-back to AgentPM

This work reinforces the central AgentPM direction:

- define reusable artifacts once
- version them cleanly
- publish them predictably
- install them securely
- support both open and internal ecosystems

Before this spec, AgentPM had a stronger story for public reuse than private internal reuse.

After this spec, it has the foundation for both:

- public ecosystem distribution
- private team registry workflows

That makes AgentPM feel less like a public catalog and more like a real packaging layer for AI systems inside teams.

## Suggested inputs for ChatGPT Projects

- Possible title ideas
  - `Why AgentPM needed private namespaces`
  - `From public registry to private team registry`
  - `What it takes to add private package access correctly`
  - `The next step for AgentPM: private namespaces and early pricing`

- Possible hook
  - “A package manager gets much more useful the moment a team can keep some packages private without losing search, install, or versioning discipline.”

- Supporting examples or screenshots worth using
  - the `/pricing` page with Free, Pro, and Team
  - the `/profile` namespace panel showing multiple namespaces and org/public choices
- org member-management UI
- a private namespace detail page for an authorized user
- a signed-in search result that can see authorized private results
- the billing section in `/profile`
