// hashmap_i64: 1,000,000 integer-key inserts, 1,000,000 present lookups,
// 1,000,000 absent lookups and 100,000 deletions in a Map; the folded
// checksum, the final size and the number of successful lookups are printed.
"use strict";
const MOD = 1000000007;

function mix(fold, value) {
    return (fold * 131 + value) % MOD;
}

function key(i) {
    return (i * 2654435761) % 2147483648;
}

function main() {
    const n = 1000000;
    const m = new Map();
    let i = 0;
    while (i < n) {
        const k = key(i);
        m.set(k, (k % 10007) + 1);
        i += 1;
    }
    let fold = 0;
    let hits = 0;
    i = 0;
    while (i < n) {
        const value = m.get(key(i));
        if (value !== undefined) {
            hits += 1;
            fold = mix(fold, value);
        }
        i += 1;
    }
    i = 0;
    while (i < n) {
        const k = 2147483648 + i;
        const value = m.get(k);
        if (value === undefined) {
            fold = mix(fold, k % 97);
        } else {
            fold = mix(fold, value);
        }
        i += 1;
    }
    let d = 0;
    while (d < 100000) {
        const removed = m.get(key(d * 10));
        if (removed !== undefined) {
            m.delete(key(d * 10));
            fold = mix(fold, removed);
        } else {
            fold = mix(fold, 1);
        }
        d += 1;
    }
    console.log("checksum=" + fold + "," + m.size + "," + hits);
}

main();
