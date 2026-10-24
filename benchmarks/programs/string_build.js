"use strict";

function main() {
    var tokens = 1000000;
    var parts = [];
    var token_sum = 0;
    for (var i = 0; i < tokens; i++) {
        var v = i % 1000;
        token_sum += v;
        parts.push(String(v));
    }
    var s = parts.join(",") + ",";
    console.log("checksum=" + s.length + "," + token_sum);
}

main();
