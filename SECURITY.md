# Security Policy

Lithos operates against real Roblox accounts, state backends, and deployment credentials. Treat anything that could leak credentials, corrupt remote state, bypass auth checks, or perform unintended destructive changes as a security issue.

## Supported versions

Security fixes are handled on a best-effort basis for:

- the latest tagged release
- the current `main` branch

Older versions may be fixed when the change is small and low-risk, but that should not be assumed.

## Do not report vulnerabilities in public

Do **not** open a public issue, pull request, or discussion for a suspected vulnerability.

That includes reports about:

- credential leakage
- auth bypasses
- unsafe state handling
- unintended destructive deploy behavior with security impact
- exposed secrets in examples, fixtures, docs, or release artifacts

## Private reporting path

The only repository-verified security contact currently available is the repository owner, [@siriuslatte](https://github.com/siriuslatte).

Send the report through a private contact method that is currently published on that GitHub profile. Do not include vulnerability details anywhere public while you are trying to establish contact.

## What to include

Please include as much of the following as you can:

- affected version, commit, or branch
- impact and attack scenario
- reproduction steps or proof of concept
- whether the issue requires credentials, specific scopes, or a particular target setup
- whether the problem affects local-only workflows, live Roblox API calls, remote state, or release artifacts

## Response expectations

This repository does not publish a guaranteed response SLA. Reports are handled on a best-effort basis.

The maintainer will try to:

- acknowledge receipt when possible
- reproduce and scope the issue
- coordinate a fix before public disclosure

Please keep the report private until the maintainer confirms that public disclosure is safe.