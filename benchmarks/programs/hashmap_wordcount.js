// hashmap_wordcount: count 200,000 deterministic words (4,096 distinct) in a
// Map keyed by the word text, then print a checksum over the frequencies, the
// number of distinct keys and the highest frequency. The checksum (sum of
// squared counts) is independent of Map iteration order.
"use strict";

function main() {
    var tokens = 200000;
    var distinct = 4096;
    var counts = new Map();
    for (var i = 0; i < tokens; i++) {
        var word = "w" + (i % distinct);
        counts.set(word, (counts.get(word) || 0) + 1);
    }
    var checksum = 0;
    var maxCount = 0;
    for (var count of counts.values()) {
        checksum += count * count;
        if (count > maxCount) {
            maxCount = count;
        }
    }
    console.log("checksum=" + checksum + "," + counts.size + "," + maxCount);
}

main();
