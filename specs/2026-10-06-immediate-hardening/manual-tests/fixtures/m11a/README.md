# M11A Local Fixture

This disposable Tool is for lint and author-side dependency-resolution checks.
It must not be published. Its entrypoint only prints JSON; it does not import
the declared third-party packages. M11A prepares dependency state, not a managed
environment. These fixtures do not change what `agentpm init` creates.
