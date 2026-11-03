"""Independent reference implementations ("oracles") for the benchmark suite.

Every benchmark program prints one deterministic checksum line.  The values in
this module are computed from the workload descriptions and from arithmetic
alone - they are NOT produced by running any of the benchmark programs.  The
harness compares each program's expected output with the oracle before it
measures anything, so a checksum can never silently drift when a source file is
edited.

The module is also the single source of truth for the data scales: SCALES maps
each scalable workload to the argument list and metadata of its small, medium
and large size, and expected(program, args) returns the exact stdout line
(without a trailing newline) for the given arguments.

Only the Python standard library is used.
"""

MODULUS = 1000000007
INT64_MODULUS = 2147483648
LCG_MODULUS = 2147483647
MANDELBROT_MAX_ITER = 100


# ---------------------------------------------------------------------------
# small numeric helpers
# ---------------------------------------------------------------------------


def _mix(fold, value):
    """Fold one value into the running checksum (same recurrence everywhere)."""
    return (fold * 131 + value) % MODULUS


def _ints(args, defaults):
    """Parse integer arguments, falling back to defaults for missing ones."""
    values = [int(text) for text in args]
    if len(values) > len(defaults):
        raise ValueError("too many arguments: %r" % (args,))
    return values + list(defaults[len(values):])


def _fib(n):
    """Iterative Fibonacci so the oracle stays cheap even for large n."""
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def _prime_count(limit):
    """Number of primes <= limit (plain sieve of Eratosthenes)."""
    if limit < 2:
        return 0
    flags = bytearray([1]) * (limit + 1)
    flags[0] = 0
    flags[1] = 0
    i = 2
    while i * i <= limit:
        if flags[i]:
            flags[i * i : limit + 1 : i] = bytes((limit - i * i) // i + 1)
        i += 1
    return sum(flags)


def _lcg_values(n, seed=12345):
    """Park-Miller LCG sequence used by the sorting workloads."""
    values = []
    state = seed
    for _ in range(n):
        state = (16807 * state) % LCG_MODULUS
        values.append(state)
    return values


def _sorted_checksum(n, seed=12345):
    """First, last and sum of the sorted LCG sequence."""
    values = sorted(_lcg_values(n, seed))
    return "checksum=%d,%d,%d" % (values[0], values[-1], sum(values))


def _sequential_string_stats(tokens):
    """Length in bytes and value sum of the string '0,1,...,999,0,1,...'."""
    full, rest = divmod(tokens, 1000)
    cycle_sum = sum(range(1000))
    cycle_len = sum(len(str(v)) + 1 for v in range(1000))
    token_sum = full * cycle_sum + sum(range(rest))
    length = full * cycle_len + sum(len(str(v)) + 1 for v in range(rest))
    return length, token_sum


def _count_word_frequencies(tokens, distinct):
    """Frequency statistics of the deterministic word stream w0..w4095..."""
    full, rest = divmod(tokens, distinct)
    checksum = rest * (full + 1) * (full + 1) + (distinct - rest) * full * full
    max_count = full + 1 if rest else full
    return checksum, min(tokens, distinct), max_count


# ---------------------------------------------------------------------------
# individual workloads (one function per program)
# ---------------------------------------------------------------------------


def _fib_output(args):
    n, repetitions = _ints(args, (30, 10))
    return "checksum=%d" % (_fib(n) * repetitions)


def _fib_iter_output(args):
    # Identical specification to fib, different implementation in the program.
    return _fib_output(args)


def _fib_memo_output(args):
    return _fib_output(args)


def _loop_sum_output(args):
    (n,) = _ints(args, (8000000,))
    return "checksum=%d" % (n * (n + 1) // 2)


def _string_build_output(args):
    (tokens,) = _ints(args, (1000000,))
    length, token_sum = _sequential_string_stats(tokens)
    return "checksum=%d,%d" % (length, token_sum)


def _vec_memory_output(args):
    (n,) = _ints(args, (1000000,))
    return "checksum=%d" % (3 * n * (n - 1) // 2)


def _sort_i64_output(args):
    (n,) = _ints(args, (200000,))
    return _sorted_checksum(n)


def _sort_insertion_output(args):
    # Insertion sort produces the same sorted sequence as the quicksort, so the
    # checksum only differs when the size differs.
    (n,) = _ints(args, (1000,))
    return _sorted_checksum(n)


def _matrix_multiply_output(args):
    (n,) = _ints(args, (160,))
    # total = sum_k (sum_i A[i][k]) * (sum_j B[k][j]) - an algebraic
    # rearrangement that avoids materialising the n^3 products in Python.
    total = 0
    for k in range(n):
        left = sum((i * 31 + k * 17) % 1000 for i in range(n))
        right = sum((k * 13 + j * 7) % 1000 for j in range(n))
        total += left * right
    return "checksum=%d" % total


def _binary_trees_output(args):
    (depth,) = _ints(args, (16,))
    leaves = 1 << depth
    return "checksum=%d" % (leaves * (leaves - 1) // 2)


def _string_split_output(args):
    (tokens,) = _ints(args, (50000,))
    length, token_sum = _sequential_string_stats(tokens)
    return "checksum=%d,%d,%d" % (length, tokens, token_sum)


def _sieve_output(args):
    (limit,) = _ints(args, (2000000,))
    return "checksum=%d" % _prime_count(limit)


def _mandelbrot_output(args):
    width, height = _ints(args, (600, 400))
    total = 0
    for py in range(height):
        ci = (float(py) / float(height)) * 2.0 - 1.0
        for px in range(width):
            cr = (float(px) / float(width)) * 3.5 - 2.5
            zx = 0.0
            zy = 0.0
            iteration = 0
            while iteration < MANDELBROT_MAX_ITER:
                zx2 = zx * zx
                zy2 = zy * zy
                if zx2 + zy2 > 4.0:
                    break
                zy = 2.0 * zx * zy + ci
                zx = zx2 - zy2 + cr
                iteration += 1
            total += iteration
    return "checksum=%d" % total


def _iterator_pipeline_output(args):
    (n,) = _ints(args, (1000000,))
    even = n // 2  # values 0, 2, 4, ...; their squares are 4*k^2
    total = 4 * (even - 1) * even * (2 * even - 1) // 6
    return "checksum=%d" % total


def _hashmap_wordcount_output(args):
    tokens, distinct = _ints(args, (200000, 4096))
    checksum, distinct_count, max_count = _count_word_frequencies(tokens, distinct)
    return "checksum=%d,%d,%d" % (checksum, distinct_count, max_count)


def _array_wordcount_output(args):
    # Same word stream and same statistics as hashmap_wordcount; the program
    # uses a flat Vec instead of the builtin HashMap.
    return _hashmap_wordcount_output(args)


def _generic_stack_output(args):
    integers, strings = _ints(args, (1000000, 1000))
    total = integers * (integers - 1) // 2
    char_total = sum(len("item%d" % (j % 100)) for j in range(strings))
    return "checksum=%d,%d" % (total, char_total)


def _fn_pipeline_output(args):
    def add3(x):
        return x + 3

    def modstep(x):
        return (x * 3 + 1) % 1000003

    def collatz(x):
        return x // 2 if x % 2 == 0 else x * 3 + 1

    labels = ["add3", "modstep", "collatz"]
    functions = [add3, modstep, collatz]

    def run(index, seed, times):
        value = seed
        for _ in range(times):
            value = functions[index](value)
        return value

    checksum = run(0, 11, 500) + run(0, 7, 1000) + run(1, 7, 1000) + run(2, 7, 1000)
    label_bytes = len(labels[0]) + sum(len(label) for label in labels)
    return "checksum=%d,%d" % (checksum, label_bytes)


def _string_unicode_output(args):
    (n,) = _ints(args, (120000,))
    alphabet = [
        chr(0x0061),  # a
        chr(0x00E9),  # e-acute
        chr(0x00F1),  # n-tilde
        chr(0x00FC),  # u-umlaut
        chr(0x00F6),  # o-umlaut
        chr(0x00DF),  # sharp s
        chr(0xFB01),  # fi ligature
        chr(0x00E0),  # a-grave
        chr(0x00E7),  # c-cedilla
        chr(0x03B1),  # alpha
        chr(0x03B2),  # beta
        chr(0x03B3),  # gamma
        chr(0x03B4),  # delta
        chr(0x0430),  # cyrillic a
        chr(0x0431),  # cyrillic be
        chr(0x0432),  # cyrillic ve
        chr(0x4F60),  # CJK ni
        chr(0x597D),  # CJK hao
        chr(0x4E16),  # CJK shi
        chr(0x754C),  # CJK jie
        chr(0x1F600),  # grinning face
        chr(0x006B),  # k
        chr(0x0071),  # q
        chr(0x0077),  # w
    ]
    k = len(alphabet)
    base_chars = [alphabet[i % k] for i in range(n)]
    base_bytes = "".join(base_chars).encode("utf-8")

    fold = 0
    for char in base_chars:
        fold = _mix(fold, ord(char))
    for probe in range(1000):
        fold = _mix(fold, ord(base_chars[probe * 7]))

    upper = "".join(base_chars).upper()
    upper_chars = list(upper)
    for char in upper_chars:
        fold = _mix(fold, ord(char))

    def is_char_boundary(position):
        return position == 0 or position == len(base_bytes) or (base_bytes[position] & 0xC0) != 0x80

    mid = len(base_bytes) // 2
    while not is_char_boundary(mid):
        mid -= 1
    end = mid + 5000
    while not is_char_boundary(end):
        end -= 1
    built_chars = list(base_bytes[mid:end].decode("utf-8"))
    built_chars.append("!")
    for index in range(1000):
        built_chars.append(upper_chars[index])
    popped = built_chars.pop()
    fold = _mix(fold, ord(popped))
    built = "".join(built_chars) + "done"
    for char in built:
        fold = _mix(fold, ord(char))
    return "checksum=%d,%d,%d" % (len(built), len(built.encode("utf-8")), fold)


def _hashmap_i64_output(args):
    (n,) = _ints(args, (1000000,))

    def key_at(index):
        return (index * 2654435761) % INT64_MODULUS

    values = {}
    for index in range(n):
        key = key_at(index)
        values[key] = (key % 10007) + 1
    fold = 0
    hits = 0
    for index in range(n):
        fold = _mix(fold, values[key_at(index)])
        hits += 1
    for index in range(n):
        absent = INT64_MODULUS + index
        if absent in values:
            fold = _mix(fold, values[absent])
        else:
            fold = _mix(fold, absent % 97)
    deletes = n // 10
    for index in range(deletes):
        key = key_at(index * 10)
        if key in values:
            fold = _mix(fold, values.pop(key))
        else:
            fold = _mix(fold, 1)
    return "checksum=%d,%d,%d" % (fold, len(values), hits)


def _hashmap_string_output(args):
    (tokens,) = _ints(args, (200000,))

    def key_at(index):
        return "k%d-%d" % (index % 1000, index)

    counts = {}
    for index in range(tokens):
        counts[key_at(index)] = (index % 10007) + 1
    fold = 0
    hits = 0
    for index in range(tokens):
        fold = _mix(fold, counts[key_at(index)])
        hits += 1
    for index in range(tokens):
        absent = key_at(tokens + index)
        if absent in counts:
            fold = _mix(fold, counts[absent])
        else:
            for char in absent:
                fold = _mix(fold, ord(char))
    deletes = tokens // 10
    for index in range(deletes):
        key = key_at(index * 10)
        if key in counts:
            fold = _mix(fold, counts.pop(key))
        else:
            fold = _mix(fold, 1)
    return "checksum=%d,%d,%d" % (fold, len(counts), hits)


_IMPLEMENTATIONS = {
    "fib": _fib_output,
    "fib_iter": _fib_iter_output,
    "fib_memo": _fib_memo_output,
    "loop_sum": _loop_sum_output,
    "string_build": _string_build_output,
    "vec_memory": _vec_memory_output,
    "sort_i64": _sort_i64_output,
    "sort_insertion": _sort_insertion_output,
    "matrix_multiply": _matrix_multiply_output,
    "binary_trees": _binary_trees_output,
    "binary_trees_refs": _binary_trees_output,
    "string_split": _string_split_output,
    "sieve": _sieve_output,
    "mandelbrot": _mandelbrot_output,
    "iterator_pipeline": _iterator_pipeline_output,
    "hashmap_wordcount": _hashmap_wordcount_output,
    "array_wordcount": _array_wordcount_output,
    "generic_stack": _generic_stack_output,
    "fn_pipeline": _fn_pipeline_output,
    "string_unicode": _string_unicode_output,
    "hashmap_i64": _hashmap_i64_output,
    "hashmap_string": _hashmap_string_output,
}


# ---------------------------------------------------------------------------
# data scales
# ---------------------------------------------------------------------------
# Each scalable workload has three sizes.  An empty argument list means "the
# program's own default" (the medium size), so the harness reuses the primary
# measurement instead of running the same thing twice.  The sizes are chosen so
# that the large variants stay under roughly a second and a half in the slow
# interpreted languages while still separating small from large by at least an
# order of magnitude in work.

SCALES = {
    "fib": {
        "small": {"args": ["20", "10"], "workload": {"n": 20, "repetitions": 10}},
        "medium": {"args": [], "workload": {"n": 30, "repetitions": 10}},
        "large": {"args": ["31", "10"], "workload": {"n": 31, "repetitions": 10}},
    },
    "fib_iter": {
        "small": {"args": ["20", "10"], "workload": {"n": 20, "repetitions": 10}},
        "medium": {"args": [], "workload": {"n": 30, "repetitions": 10}},
        "large": {"args": ["31", "10"], "workload": {"n": 31, "repetitions": 10}},
    },
    "fib_memo": {
        "small": {"args": ["20", "10"], "workload": {"n": 20, "repetitions": 10}},
        "medium": {"args": [], "workload": {"n": 30, "repetitions": 10}},
        "large": {"args": ["31", "10"], "workload": {"n": 31, "repetitions": 10}},
    },
    "loop_sum": {
        "small": {"args": ["1000000"], "workload": {"n": 1000000}},
        "medium": {"args": [], "workload": {"n": 8000000}},
        "large": {"args": ["12000000"], "workload": {"n": 12000000}},
    },
    "string_build": {
        "small": {"args": ["10000"], "workload": {"tokens": 10000}},
        "medium": {"args": [], "workload": {"tokens": 1000000}},
        "large": {"args": ["4000000"], "workload": {"tokens": 4000000}},
    },
    "sort_i64": {
        "small": {"args": ["1000"], "workload": {"n": 1000}},
        "medium": {"args": [], "workload": {"n": 200000}},
        "large": {"args": ["500000"], "workload": {"n": 500000}},
    },
    "sort_insertion": {
        "small": {"args": ["250"], "workload": {"n": 250}},
        "medium": {"args": [], "workload": {"n": 1000}},
        "large": {"args": ["4000"], "workload": {"n": 4000}},
    },
    "binary_trees": {
        "small": {"args": ["10"], "workload": {"depth": 10}},
        "medium": {"args": [], "workload": {"depth": 16}},
        "large": {"args": ["18"], "workload": {"depth": 18}},
    },
    "binary_trees_refs": {
        "small": {"args": ["10"], "workload": {"depth": 10}},
        "medium": {"args": [], "workload": {"depth": 16}},
        "large": {"args": ["18"], "workload": {"depth": 18}},
    },
    "string_split": {
        "small": {"args": ["1000"], "workload": {"tokens": 1000}},
        "medium": {"args": [], "workload": {"tokens": 50000}},
        "large": {"args": ["500000"], "workload": {"tokens": 500000}},
    },
    "sieve": {
        "small": {"args": ["100000"], "workload": {"limit": 100000}},
        "medium": {"args": [], "workload": {"limit": 2000000}},
        "large": {"args": ["5000000"], "workload": {"limit": 5000000}},
    },
    "hashmap_wordcount": {
        "small": {"args": ["10000"], "workload": {"tokens": 10000, "alphabet": 4096}},
        "medium": {"args": [], "workload": {"tokens": 200000, "alphabet": 4096}},
        "large": {"args": ["1000000"], "workload": {"tokens": 1000000, "alphabet": 4096}},
    },
    "array_wordcount": {
        "small": {"args": ["10000"], "workload": {"tokens": 10000, "alphabet": 4096}},
        "medium": {"args": [], "workload": {"tokens": 200000, "alphabet": 4096}},
        "large": {"args": ["1000000"], "workload": {"tokens": 1000000, "alphabet": 4096}},
    },
}

SCALE_NAMES = ("small", "medium", "large")

SCALABLE_PROGRAMS = tuple(SCALES)


def expected(program, args=None):
    """Return the exact stdout line (no trailing newline) for a program."""
    implementation = _IMPLEMENTATIONS.get(program)
    if implementation is None:
        raise KeyError("no oracle for benchmark program %r" % (program,))
    return implementation(list(args or ()))


def expected_scales(program):
    """Return {scale: expected_line} for a scalable program."""
    return {
        scale: expected(program, spec["args"])
        for scale, spec in SCALES[program].items()
    }


def oracle_programs():
    """Names of every program this module can verify."""
    return tuple(sorted(_IMPLEMENTATIONS))
