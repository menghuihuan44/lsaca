// binary_trees: build a perfect binary tree (leaves carry consecutive
// payloads), recursively sum the payloads.
// Optional argument: depth (default 16, the medium scale).
'use strict';

class Node {
  constructor(value, left, right) {
    this.value = value;
    this.left = left;
    this.right = right;
  }
}

function build(depth, start) {
  if (depth === 0) {
    return new Node(start, null, null);
  }
  return new Node(0, build(depth - 1, start), build(depth - 1, start + (1 << (depth - 1))));
}

function sumTree(node) {
  if (node === null) {
    return 0;
  }
  return node.value + sumTree(node.left) + sumTree(node.right);
}

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const depth = argInt(process.argv.slice(2), 0, 16);
console.log('checksum=' + sumTree(build(depth, 0)));