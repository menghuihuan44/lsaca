// string_build: append numeric tokens to a string.
// Optional argument: token count; the default is the medium scale.
'use strict';

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const tokens = argInt(process.argv.slice(2), 0, 1000000);
const parts = [];
let tokenSum = 0;
for (let i = 0; i < tokens; i++) {
  const v = i % 1000;
  tokenSum += v;
  parts.push(String(v));
}
const s = parts.join(',') + ',';
console.log('checksum=' + Buffer.byteLength(s, 'utf8') + ',' + tokenSum);