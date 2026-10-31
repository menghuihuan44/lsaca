// string_unicode: exercise the UTF-8 string API over a deterministic
// mixed-ASCII/Unicode corpus: push 120,000 scalars one at a time, fold the
// scalars from iteration and code-point access, apply full-Unicode
// toUpperCase, slice with byte offsets at character boundaries, extend, pop
// and fold again. The checksum is <char_count>,<byte_len>,<folded checksum>.
"use strict";
const MOD = 1000000007;

function mix(fold, value) {
    return (fold * 131 + value) % MOD;
}

function main() {
    const alphabet = Array.from("a\u00e9\u00f1\u00fc\u00f6\u00df\ufb01\u00e0\u00e7\u03b1\u03b2\u03b3\u03b4\u0430\u0431\u0432\u4f60\u597d\u4e16\u754c\u{1F600}kqw").map(c => c.codePointAt(0));
    const n = 120000;
    const k = alphabet.length;
    const base = [];
    let i = 0;
    while (i < n) {
        base.push(alphabet[i % k]);
        i += 1;
    }
    let fold = 0;
    for (const cp of base) {
        fold = mix(fold, cp);
    }
    let probes = 0;
    while (probes < 1000) {
        fold = mix(fold, base[probes * 7]);
        probes += 1;
    }
    const upper = [];
    for (const cp of base) {
        for (const ch of String.fromCodePoint(cp).toUpperCase()) {
            upper.push(ch.codePointAt(0));
        }
    }
    for (const cp of upper) {
        fold = mix(fold, cp);
    }
    const offsets = [];
    let off = 0;
    for (const cp of base) {
        offsets.push(off);
        off += Buffer.byteLength(String.fromCodePoint(cp), "utf8");
    }
    offsets.push(off);
    const data = Buffer.from(base.map(cp => String.fromCodePoint(cp)).join(""), "utf8");
    let mid = Math.floor(off / 2);
    while (!offsets.includes(mid)) {
        mid -= 1;
    }
    let end = mid + 5000;
    while (!offsets.includes(end)) {
        end -= 1;
    }
    let built = data.subarray(mid, end).toString("utf8");
    built += "!";
    built += upper.slice(0, 1000).map(cp => String.fromCodePoint(cp)).join("");
    const chars = Array.from(built);
    const popped = chars.pop();
    fold = mix(fold, popped.codePointAt(0));
    built = chars.join("") + "done";
    for (const ch of built) {
        fold = mix(fold, ch.codePointAt(0));
    }
    console.log("checksum=" + Array.from(built).length + "," + Buffer.byteLength(built, "utf8") + "," + fold);
}

main();
