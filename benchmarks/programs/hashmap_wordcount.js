// hashmap_wordcount: count deterministic words (4,096 distinct) in a Map
// keyed by the word text, then print a checksum over the frequencies, the
// number of distinct keys and the highest frequency.
// Optional argument: token count (default 200,000, the medium scale).
'use strict';

function argInt(args, index, fallback) {
  if (index < args.length) {
    return Number.parseInt(args[index], 10);
  }
  return fallback;
}

const tokens = argInt(process.argv.slice(2), 0, 200000);
const distinct = 4096;
const counts = new Map();
for (let i = 0; i < tokens; i++) {
  const word = 'w' + (i % distinct);
  counts.set(word, (counts.get(word) || 0) + 1);
}
let checksum = 0;
let maxCount = 0;
for (const count of counts.values()) {
  checksum += count * count;
  if (count > maxCount) {
    maxCount = count;
  }
}
console.log('checksum=' + checksum + ',' + counts.size + ',' + maxCount);