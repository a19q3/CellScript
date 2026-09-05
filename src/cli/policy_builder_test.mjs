import assert from "node:assert/strict";
import test from "node:test";
import * as builder from "../dist/index.js";

const magic = [0x43, 0x53, 0x41, 0x52, 0x47, 0x76, 0x31, 0];
const golden = [
  "4353504f4c763100", "8f0000000c00000052000000",
  "4600000014000000150000003500000039000000", "00",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "07000000090000004353415247763100aa",
  "3d00000014000000150000003500000039000000", "01",
  "1111111111111111111111111111111111111111111111111111111111111111",
  "0403020100000000",
].join("");

test("policy codec matches the independent canonical Molecule golden", () => {
  const records = [
    { role: 1, scriptHash: new Uint8Array(32).fill(0x11), tag: 0x01020304, args: new Uint8Array() },
    { role: 0, scriptHash: new Uint8Array(32), tag: 7, args: Uint8Array.from([...magic, 0xaa]) },
  ];
  assert.equal(Buffer.from(builder.encodePolicyWitnessBundle(records)).toString("hex"), golden);
  assert.throws(() => builder.encodePolicyWitnessBundle([]), /1 through 8/);
  assert.throws(() => builder.encodePolicyWitnessBundle(Array(9).fill(records[0])), /1 through 8/);
  assert.throws(() => builder.encodePolicyWitnessBundle([records[0], { ...records[0], tag: 8 }]), /Duplicate/);
  for (const field of [{ role: 2 }, { tag: -1 }, { tag: 1.5 }, { tag: 0x100000000 }, { scriptHash: new Uint8Array(31) }, { args: Uint8Array.of(1) }]) {
    assert.throws(() => builder.encodePolicyWitnessBundle([{ ...records[0], ...field }]));
  }
  const args = new Uint8Array(4076 - 8 - 8 - 61);
  args.set(magic);
  assert.equal(builder.encodePolicyWitnessBundle([{ ...records[0], args }]).length, 4076);
  const tooLarge = new Uint8Array(args.length + 1);
  tooLarge.set(args);
  assert.throws(() => builder.encodePolicyWitnessBundle([{ ...records[0], args: tooLarge }]), /4076/);
});

test("exported policy action routing is explicit and never authenticates", () => {
  for (const action of builder.builderManifest.actions) {
    const request = builder.policyWitnessRequest(action.name);
    assert.equal(request.tag, action.policy_tag);
    assert.equal(request.requiresPolicyWitnessBundle, true);
    assert.equal(request.requiresPreSigningPlacement, true);
    assert.equal(request.typedArgsEncodedByHelper, false);
    assert.equal(request.authenticatesCaller, false);
    // Structural framing only: this intentionally does not assert that a
    // header-only CSARG buffer satisfies a selected nonempty argument schema.
    const args = action.entry_witness_required ? Uint8Array.from(magic) : new Uint8Array();
    const record = builder.createPolicyWitnessRecord(action.name, new Uint8Array(32), args);
    assert.equal(record.tag, action.policy_tag);
    assert.equal(record.role, 1);
    assert.deepEqual(record.args, args);
    assert.throws(() => builder.createPolicyWitnessRecord(action.name, new Uint8Array(31), args));
    assert.throws(() => builder.createPolicyWitnessRecord(action.name, new Uint8Array(32), action.entry_witness_required ? new Uint8Array() : Uint8Array.from(magic)));
  }
  assert.throws(() => builder.policyWitnessRequest("__not_an_exported_policy_action__"));
  for (const inherited of ["__proto__", "constructor", "toString"]) {
    if (!builder.builderManifest.actions.some(action => action.name === inherited)) {
      assert.throws(() => builder.policyWitnessRequest(inherited));
    }
  }
  for (const name of builder.policyArtifact.declaration.common_checks) {
    assert.throws(() => builder.createPolicyWitnessRecord(name, new Uint8Array(32), new Uint8Array()));
  }
  for (const [name, fn] of Object.entries(builder)) {
    if (!name.startsWith("plan") || typeof fn !== "function") continue;
    const plan = fn({});
    assert.deepEqual(plan.policyWitness, builder.policyWitnessRequest(plan.action));
  }
});
