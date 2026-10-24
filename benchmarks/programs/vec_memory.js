"use strict";

function main() {
    var n = 1000000;
    var points = [];
    for (var i = 0; i < n; i++) {
        points.push({ x: i, y: 2 * i });
    }
    var checksum = 0;
    for (var j = 0; j < n; j++) {
        checksum += points[j].x + points[j].y;
    }
    console.log("checksum=" + checksum);
}

main();
