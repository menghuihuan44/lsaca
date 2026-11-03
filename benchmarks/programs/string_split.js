// string_split: build a comma-separated string of decimal numbers, split it,
// parse every field and sum it.
// Optional argument: token count (default 50,000, the medium scale).
'use strict';

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const tokens = argInt(process.argv.slice(2), 0, 50000);
const pieces = [];
for (let i = 0; i < tokens; i++) {
  pieces.push(String(i % 1000) + ',');
}
const text = pieces.join('');
let count = 0;
let total = 0;
for (const part of text.split(',')) {
  if (part !== '') {
    total += Number.parseInt(part, 10);
    count += 1;
  }
}
console.log('checksum=' + Buffer.byteLength(text, 'utf8') + ',' + count + ',' + total);