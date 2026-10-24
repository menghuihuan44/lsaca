"use strict";

function main() {
    var n = 8000000;
    var sum = 0;
    for (var i = 1; i <= n; i++) {
        sum += i;
    }
    console.log("checksum=" + sum);
}

main();
