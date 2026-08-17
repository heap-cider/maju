import assert from "node:assert/strict";
import test from "node:test";

import remarkEntityLinks from "./remarkEntityLinks.ts";

function run(value) {
  const tree = {
    type: "root",
    children: [{ type: "paragraph", children: [{ type: "text", value }] }],
  };
  remarkEntityLinks()(tree);
  return tree.children[0].children;
}

test("turns every bare Maju entity permalink family into a chip node", () => {
  const owner = "ab".repeat(32);
  const id = "cd".repeat(32);
  const links = [
    `maju://repo?owner=${owner}&d=maju`,
    `maju://pr?id=${id}&owner=${owner}&d=maju`,
    `maju://issue?id=${id}&owner=${owner}&d=maju`,
  ];
  for (const link of links) {
    const children = run(link);
    assert.equal(children[0].type, "entity-link");
    assert.equal(children[0].value, link);
  }
});

test("keeps sentence punctuation outside entity chip nodes", () => {
  const link = `maju://repo?owner=${"ab".repeat(32)}&d=maju`;
  const children = run(`${link}.`);
  assert.equal(children[0].value, link);
  assert.equal(children[1].value, ".");
});
