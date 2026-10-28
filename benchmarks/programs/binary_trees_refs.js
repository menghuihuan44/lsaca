// binary_trees: build a perfect binary tree of depth 16 (131,071 nodes, leaves
// carry consecutive payloads), recursively sum the payloads.
const DEPTH = 16;

function build(depth, start) {
  if (depth === 0) {
    return { value: start, left: null, right: null };
  }
  return {
    value: 0,
    left: build(depth - 1, start),
    right: build(depth - 1, start + (1 << (depth - 1))),
  };
}

function sumTree(node) {
  if (node === null) return 0;
  return node.value + sumTree(node.left) + sumTree(node.right);
}

function main() {
  console.log(`checksum=${sumTree(build(DEPTH, 0))}`);
}

main();
