// hashmap_string: 200,000 distinct generated String keys inserted into a Map,
// then 200,000 present lookups, 200,000 absent lookups and 20,000 deletions;
// the folded checksum, the final size and the number of successful lookups
// are printed.
"use strict";
const MOD = 1000000007;

function mix(fold, value) {
    return (fold * 131 + value) % MOD;
}

function key(i) {
    return "k" + (i % 1000) + "-" + i;
}

function main() {
    const tokens = 200000;
    const m = new Map();
    let i = 0;
    while (i < tokens) {
        m.set(key(i), (i % 10007) + 1);
        i += 1;
    }
    let fold = 0;
    let hits = 0;
    i = 0;
    while (i < tokens) {
        const value = m.get(key(i));
        if (value !== undefined) {
            hits += 1;
            fold = mix(fold, value);
        }
        i += 1;
    }
    i = 0;
    while (i < tokens) {
        const absent = key(tokens + i);
        const value = m.get(absent);
        if (value === undefined) {
            for (const byte of Buffer.from(absent, "utf8")) {
                fold = mix(fold, byte);
            }
        } else {
            fold = mix(fold, value);
        }
        i += 1;
    }
    let d = 0;
    while (d < 20000) {
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
