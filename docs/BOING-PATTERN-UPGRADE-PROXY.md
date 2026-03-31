# Pattern: Upgradeable contracts, proxies, and QA

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **F3**.

---

## Protocol stance

- **Contract code** on Boing is deployed as **bytecode at an `AccountId`**. There is **no** EVM `DELEGATECALL` opcode in the Boing VM spec as of the current execution parity list.
- **Immutability by default:** Once deployed and accepted by QA, **bytecode is fixed** for that account unless the protocol adds an explicit **upgrade** mechanism later.

---

## Allowed app-level patterns (conceptual)

1. **Registry / pointer contract**  
   A small **immutable** “hub” contract stores an `AccountId` of the **current implementation** in storage. Users call the hub; the hub **`ContractCall`**s the implementation. **Upgrades** = deploy **new** implementation account + **`ContractCall`** to hub to **rotate pointer** (access-controlled).  
   - Each **new** implementation bytecode is a **new deploy** → passes **QA** again.

2. **Migration**  
   Deploy **`v2`** alongside **`v1`**; state is migrated via explicit txs; **`v1`** is abandoned or frozen in logic. No proxy trick required.

---

## Patterns that conflict with QA intent

- **Opaque delegate to mutable off-chain bytecode** or **hidden implementation swaps** designed to **evade** QA review of what users interact with are **against the spirit** of the pillar: users and the network should understand **what code** runs at which address.
- If a pattern is used to **bypass** purpose declarations or to deploy **unreviewed** execution paths, governance should treat it under **scam-pattern** and **legitimacy** rules ([QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)).

---

## Deployer checklist

- Document **upgrade policy** in metadata / off-chain spec (who can rotate pointer, timelock, multisig).
- Every **new** implementation: **`boing_qaCheck`** + valid **purpose category**.
- Access lists: hub + implementation + any token accounts touched.

---

## References

- [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) — Appendix D (summary pointer)
- [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md)
