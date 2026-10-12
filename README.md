# Tether

A LAN-only remote mouse and keyboard application.

The desktop application hosts a local mobile web interface. A phone or
tablet on the same LAN connects through its browser and controls the
computer's mouse and keyboard.

Supported desktop platforms:

- Windows
- macOS
- Linux

## Project status

The architecture review is accepted and reconciled into `SPEC.md`. The manual
environment inventory and verified repository/toolchain skeleton are complete.
The platform-neutral input domain, normalization, mock backend, bounded
fail-safe dispatcher, settings/installation-identity storage, canonical key
mapping, and inert backend conformance suite are implemented. Native backends,
sessions, networking, and user interfaces proceed through the
dependency-ordered plan.

- [Architecture and feasibility review](docs/architecture-review.md)
- [Accepted architecture decisions](docs/architecture-decisions.md)
- [Open risks and validation gates](docs/open-risks.md)
- [Repository structure](docs/repository-structure.md)
- [Component documentation plan](docs/documentation-plan.md)
- [Dependency-ordered implementation plan](docs/implementation-plan.md)
- [Dependency and supply-chain policy](docs/dependency-policy.md)
- [Initial dependency and license inventory](docs/dependency-inventory.md)
- [Agent development instructions](AGENTS.md)
