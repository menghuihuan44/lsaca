# fn_pipeline: rules are records that carry a non-copy label string and a
# function value in the same struct. The runner borrows a rule and calls its
# function value 1,000 times; the checksum sweeps three rules plus a clone of the
# first one, and reports the total label length.
class Rule:
    def __init__(self, label, apply):
        self.label = label
        self.apply = apply


def run(rule, seed, times):
    value = seed
    for _ in range(times):
        value = rule.apply(value)
    return value


def main():
    rules = [
        Rule("add3", lambda x: x + 3),
        Rule("modstep", lambda x: (x * 3 + 1) % 1000003),
        Rule("collatz", lambda x: x // 2 if x % 2 == 0 else x * 3 + 1),
    ]
    checksum = 0
    label_bytes = 0
    for rule in rules:
        checksum += run(rule, 7, 1000)
        label_bytes += len(rule.label)
    copy = rules[0]
    checksum += run(copy, 11, 500)
    label_bytes += len(copy.label)
    print("checksum=%d,%d" % (checksum, label_bytes))


if __name__ == "__main__":
    main()
