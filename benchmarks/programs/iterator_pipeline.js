// iterator_pipeline: build 1,000,000 i64 values, then run "keep even values,
// square them, add them up" and print the folded sum. The sum is larger than
// 2^53, so the accumulator uses BigInt to stay exact.
"use strict";

function makeNumbers(n) {
    var values = new Array(n);
    for (var i = 0; i < n; i++) {
        values[i] = i;
    }
    return values;
}

function main() {
    var values = makeNumbers(1000000);
    var total = 0n;
    for (var i = 0; i < values.length; i++) {
        var value = values[i];
        if (value % 2 === 0) {
            var big = BigInt(value);
            total += big * big;
        }
    }
    console.log("checksum=" + total.toString());
}

main();
