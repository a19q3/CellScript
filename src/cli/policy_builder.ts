
/** Canonical outer encoding only. Runtime adapters still encode typed inner
 * arguments, aggregate requests by physical witness index, preserve the lock
 * field, enforce the final 4096-byte WitnessArgs limit, and place before signing.
 * A caller-supplied full Script hash is routing identity, not authentication. */
export interface PolicyWitnessRecord {
  role: 0 | 1;
  scriptHash: Uint8Array;
  tag: number;
  args: Uint8Array;
}

export function policyWitnessRequest(action: string): Readonly<Record<string, unknown>> {
  const variant = Object.hasOwn(POLICY_VARIANTS, action) ? POLICY_VARIANTS[action] : undefined;
  if (!variant) throw new Error("Action is not an exported policy variant: " + action);
  return {
    artifact: policyArtifact.declaration.name,
    role: "type",
    tag: variant.tag,
    payloadAbi: policyArtifact.payload_abi,
    placementAbi: policyArtifact.placement_abi,
    placementField: policyArtifact.placement_field,
    placementSource: policyArtifact.placement_source,
    fullScriptHashRequired: true,
    requiresPolicyWitnessBundle: true,
    requiresSharedWitnessAggregation: true,
    requiresPreSigningPlacement: true,
    typedArgsEncodedByHelper: false,
    authenticatesCaller: false,
  };
}

/** `entryArgs` is already encoded by a schema-aware inner CSARGv1 encoder.
 * This helper validates framing and the no-payload case, not arbitrary inner
 * schema content. Never pass a code hash or an address as `fullScriptHash`. */
export function createPolicyWitnessRecord(
  action: string,
  fullScriptHash: Uint8Array,
  entryArgs: Uint8Array,
): PolicyWitnessRecord {
  const variant = Object.hasOwn(POLICY_VARIANTS, action) ? POLICY_VARIANTS[action] : undefined;
  if (!variant) throw new Error("Action is not an exported policy variant: " + action);
  policyByteArray(fullScriptHash, "full Script hash");
  if (fullScriptHash.length !== 32) throw new Error("Policy routing requires the full 32-byte Script hash");
  policyArgs(entryArgs);
  if (variant.requiresArgs !== (entryArgs.length !== 0)) {
    throw new Error("Policy action requires exactly its declared empty or CSARGv1 argument framing");
  }
  return { role: 1, scriptHash: fullScriptHash.slice(), tag: variant.tag, args: entryArgs.slice() };
}

/** Encode records for one physical witness index; not a serialized WitnessArgs.
 * Foreign Lock records are structurally accepted for cohabitation. This does
 * not implement or promise a CellScript Lock-policy dispatcher. */
export function encodePolicyWitnessBundle(records: readonly PolicyWitnessRecord[]): Uint8Array {
  if (!Array.isArray(records) || records.length < 1 || records.length > 8) {
    throw new Error("Policy bundle requires 1 through 8 records");
  }
  let total = 8 + 4 * (records.length + 1);
  for (const record of records) {
    if (!record || (record.role !== 0 && record.role !== 1)) throw new Error("Unknown policy Script role");
    policyByteArray(record.scriptHash, "full Script hash");
    if (record.scriptHash.length !== 32) throw new Error("Policy routing requires the full 32-byte Script hash");
    if (!Number.isInteger(record.tag) || record.tag < 0 || record.tag > 0xffffffff) throw new Error("Policy tag must be a u32");
    policyArgs(record.args);
    total += 61 + record.args.length;
    if (total > 4076) throw new Error("Policy bundle exceeds 4076 bytes; final WitnessArgs may allow fewer");
  }
  const sorted = [...records].sort(policyRecordOrder);
  for (let i = 1; i < sorted.length; i++) {
    if (policyRecordOrder(sorted[i - 1], sorted[i]) === 0) throw new Error("Duplicate policy role and full Script hash");
  }
  const encoded = new Uint8Array(total);
  encoded.set([0x43, 0x53, 0x50, 0x4f, 0x4c, 0x76, 0x31, 0]);
  const view = new DataView(encoded.buffer);
  const put = (offset: number, value: number): void => view.setUint32(offset, value, true);
  put(8, total - 8);
  let cursor = 8 + 4 * (sorted.length + 1);
  sorted.forEach((record, index) => {
    put(12 + 4 * index, cursor - 8);
    put(cursor, 61 + record.args.length);
    [20, 21, 53, 57].forEach((offset, field) => put(cursor + 4 + field * 4, offset));
    encoded[cursor + 20] = record.role;
    encoded.set(record.scriptHash, cursor + 21);
    put(cursor + 53, record.tag);
    put(cursor + 57, record.args.length);
    encoded.set(record.args, cursor + 61);
    cursor += 61 + record.args.length;
  });
  return encoded;
}

function policyRecordOrder(left: PolicyWitnessRecord, right: PolicyWitnessRecord): number {
  if (left.role !== right.role) return left.role - right.role;
  for (let i = 0; i < 32; i++) {
    if (left.scriptHash[i] !== right.scriptHash[i]) return left.scriptHash[i] - right.scriptHash[i];
  }
  return 0;
}

function policyByteArray(value: Uint8Array, field: string): void {
  if (!(value instanceof Uint8Array)) throw new Error("Policy " + field + " must be a Uint8Array");
}

function policyArgs(args: Uint8Array): void {
  policyByteArray(args, "args");
  if (args.length > 4076) throw new Error("Policy args exceed the bounded bundle size");
  if (args.length === 0) return;
  const magic = [0x43, 0x53, 0x41, 0x52, 0x47, 0x76, 0x31, 0];
  if (args.length < magic.length || magic.some((byte, index) => args[index] !== byte)) {
    throw new Error("Nonempty policy args require the CSARGv1 magic");
  }
}
