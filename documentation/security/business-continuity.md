# Business Continuity

What a customer keeps if the vendor becomes unavailable. Addresses ISO/IEC 27001:2022 A.5.29–5.30
and A.5.22 (monitoring of supplier services), and the "key-person" question in supplier
assessments.

## 1. No runtime dependency on the vendor

A self-hosted deployment does not call the vendor to start, run, licence-check or update. It
keeps operating unchanged if the vendor's services are offline or cease to exist. The only vendor
endpoint is the optional systemprompt.io cloud, which is used only by cloud profiles
([Data handling § 3](data-handling.md#3-where-data-can-leave-the-deployment)).

## 2. Source availability

- The complete source, its history, the CI definitions and the release tooling are public in
  this repository.
- Each released version is tagged. A release can be rebuilt from its tag with `cargo build
  --locked` against the committed lockfile and offline SQLx caches; no private build input is
  required.
- Customers can keep their own mirror of the repository and of the crates they consume.

## 3. Licence continuity

The software is licensed under the Business Source License 1.1 ([LICENSE](../../LICENSE)). Each
version converts automatically to the Apache License 2.0 four years after its first public
release. Customer use is governed by the licence and the customer's commercial agreement.

## 4. Key-person risk

The project is maintained by its author. Mitigations:

- all knowledge needed to build, test and release is encoded in the repository: the `justfile`
  recipes, `scripts/`, the workflows and the [Secure development policy](secure-development-policy.md);
- releases are reproducible from public inputs (§ 2);
- the customer's deployment does not depend on the vendor at runtime (§ 1);
- source escrow can be arranged on commercial request.
