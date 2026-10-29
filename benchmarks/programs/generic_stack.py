# generic_stack: the same stack data structure instantiated for two element
# types, i64 and str. Push one million integers, pop them all into a sum; push
# one thousand small strings, pop them all into a total byte length.
class Stack:
    def __init__(self):
        self.items = []

    def push(self, item):
        self.items.append(item)

    def pop(self):
        if not self.items:
            return None
        return self.items.pop()

    def len(self):
        return len(self.items)


def main():
    n = 1000000
    numbers = Stack()
    for i in range(n):
        numbers.push(i)
    total = 0
    while numbers.len() > 0:
        total += numbers.pop()

    words = Stack()
    for j in range(1000):
        words.push("item%d" % (j % 100))
    char_total = 0
    while words.len() > 0:
        char_total += len(words.pop())

    print("checksum=%d,%d" % (total, char_total))


if __name__ == "__main__":
    main()
