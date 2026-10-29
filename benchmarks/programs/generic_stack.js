// generic_stack: the same stack data structure instantiated for two element
// types, numbers and strings. Push one million integers, pop them all into a
// sum; push one thousand small strings, pop them all into a total byte length.
"use strict";

class Stack {
    constructor() {
        this.items = [];
    }

    push(item) {
        this.items.push(item);
    }

    pop() {
        return this.items.pop();
    }

    len() {
        return this.items.length;
    }
}

function main() {
    var n = 1000000;
    var numbers = new Stack();
    for (var i = 0; i < n; i++) {
        numbers.push(i);
    }
    var total = 0;
    while (numbers.len() > 0) {
        total += numbers.pop();
    }

    var words = new Stack();
    for (var j = 0; j < 1000; j++) {
        words.push("item" + (j % 100));
    }
    var charTotal = 0;
    while (words.len() > 0) {
        var word = words.pop();
        charTotal += word.length;
    }
    console.log("checksum=" + total + "," + charTotal);
}

main();
