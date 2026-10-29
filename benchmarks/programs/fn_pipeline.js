// fn_pipeline: a rule record carries a label string and a function value in the
// same object. run() borrows the rule and calls its function value 1,000 times;
// the checksum sweeps three rules plus a copy of the first one, and reports the
// total label byte length.
"use strict";

function Rule(label, apply) {
    this.label = label;
    this.apply = apply;
}

function run(rule, seed, times) {
    var value = seed;
    for (var i = 0; i < times; i++) {
        value = rule.apply(value);
    }
    return value;
}

function main() {
    var rules = [
        new Rule("add3", function (x) { return x + 3; }),
        new Rule("modstep", function (x) { return (x * 3 + 1) % 1000003; }),
        new Rule("collatz", function (x) { return x % 2 === 0 ? x / 2 : x * 3 + 1; }),
    ];
    var checksum = 0;
    var labelBytes = 0;
    for (var i = 0; i < rules.length; i++) {
        checksum += run(rules[i], 7, 1000);
        labelBytes += rules[i].label.length;
    }
    var copy = rules[0];
    checksum += run(copy, 11, 500);
    labelBytes += copy.label.length;
    console.log("checksum=" + checksum + "," + labelBytes);
}

main();
