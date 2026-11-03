// loop_sum: integer sum 1..=n.
// Optional argument: n; the default is the medium scale.
'use strict';

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const n = argInt(process.argv.slice(2), 0, 8000000);
let total = 0;
for (let i = 1; i <= n; i++) {
  total += i;
}
console.log('checksum=' + total);