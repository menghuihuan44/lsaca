"use strict";

function fib(n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

function main() {
    var n = 30;
    var reps = 10;
    var checksum = 0;
    for (var i = 0; i < reps; i++) {
        checksum += fib(n);
    }
    console.log("checksum=" + checksum);
}

main();
