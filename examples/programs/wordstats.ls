// wordstats.ls -- word statistics over embedded text or a file.
//
// Realistic example program with a byte-identical C counterpart (wordstats.c).
//
//   lsaca run examples/programs/wordstats.ls
//   lsaca run examples/programs/wordstats.ls -- data/sample.txt
//
// The report counts lines, long lines, bytes, words, unique words and word
// lengths, prints the most frequent words and a length histogram. A word is a
// run of ASCII letters or digits, lowercased; every other character ends it.

fn bucket(length: usize) -> usize {
    if length <= 2 {
        return 0;
    }
    if length <= 4 {
        return 1;
    }
    if length <= 6 {
        return 2;
    }
    if length <= 8 {
        return 3;
    }
    if length <= 10 {
        return 4;
    }
    return 5;
}

fn flush_word(
    current: &mut String,
    counts: &mut HashMap<String, usize>,
    lengths: &mut Vec<usize>,
    words: &mut usize,
    word_bytes: &mut usize,
) {
    if current.len() == 0 {
        return;
    }
    let size = current.len();
    *words += 1;
    *word_bytes += size;
    lengths.push(size);
    let slot = counts.get_or_insert(current.clone(), 0);
    *slot += 1;
    current.clear();
}

fn print_report(text: &str) {
    let lines: Vec<String> = text.lines();
    let mut long_lines: usize = 0;
    let mut line_index: usize = 0;
    while line_index < lines.len() {
        if lines[line_index].len() > 40 {
            long_lines += 1;
        }
        line_index += 1;
    }

    let chars: Vec<char> = text.chars();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut lengths: Vec<usize> = Vec::new();
    let mut words: usize = 0;
    let mut word_bytes: usize = 0;
    let mut current = String::new();
    let mut index: usize = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else {
            flush_word(&mut current, &mut counts, &mut lengths, &mut words, &mut word_bytes);
        }
        index += 1;
    }
    flush_word(&mut current, &mut counts, &mut lengths, &mut words, &mut word_bytes);

    // Collect the unique words and their counts, then order them by count
    // (descending) and word (ascending) with a selection sort. The order is
    // independent of the hash-map slot order.
    let mut uniques: Vec<String> = Vec::new();
    let mut frequencies: Vec<usize> = Vec::new();
    for key in counts.keys() {
        let mut copy = String::new();
        copy.push_str(key.as_str());
        uniques.push(copy);
        frequencies.push(*counts.get(key).unwrap());
    }
    let mut outer: usize = 0;
    while outer < uniques.len() {
        let mut best = outer;
        let mut inner = outer + 1;
        while inner < uniques.len() {
            if frequencies[inner] > frequencies[best]
                || (frequencies[inner] == frequencies[best] && uniques[inner] < uniques[best])
            {
                best = inner;
            }
            inner += 1;
        }
        if best != outer {
            uniques.swap(outer, best);
            frequencies.swap(outer, best);
        }
        outer += 1;
    }

    let mut longest = String::new();
    let mut longest_len: usize = 0;
    let mut word_index: usize = 0;
    while word_index < uniques.len() {
        let size = uniques[word_index].len();
        if size > longest_len || (size == longest_len && uniques[word_index] < longest) {
            longest = uniques[word_index].clone();
            longest_len = size;
        }
        word_index += 1;
    }

    let mut buckets: [usize; 6] = [0; 6];
    let mut length_index: usize = 0;
    while length_index < lengths.len() {
        let slot = bucket(lengths[length_index]);
        buckets[slot] += 1;
        length_index += 1;
    }

    println!("lines: {}", lines.len());
    println!("long lines: {}", long_lines);
    println!("bytes: {}", text.len());
    println!("words: {}", words);
    println!("unique words: {}", counts.len());
    let average = if words == 0 { 0 } else { word_bytes / words };
    println!("average word length: {}", average);
    println!("longest word: {} ({})", longest, longest_len);
    println!("top words:");
    let mut top: usize = 0;
    while top < 5 && top < uniques.len() {
        println!("  {} {}", uniques[top], frequencies[top]);
        top += 1;
    }
    println!("histogram:");
    println!("  1-2: {}", buckets[0]);
    println!("  3-4: {}", buckets[1]);
    println!("  5-6: {}", buckets[2]);
    println!("  7-8: {}", buckets[3]);
    println!("  9-10: {}", buckets[4]);
    println!("  11+: {}", buckets[5]);
    println!("word bytes: {}", word_bytes);
}

fn embedded_text() -> String {
    let mut text = String::new();
    text += "The quick brown fox jumps over the lazy dog.\n";
    text += "A quick fox is quicker than a lazy dog, but the lazy dog sleeps.\n";
    text += "Rust and C are systems languages; lsaca compiles to C.\n";
    text += "Patterns, break values and iterators make the language pleasant.\n";
    text += "The dog barks; the fox runs away!\n";
    text += "The end.\n";
    return text;
}

fn main() {
    println!("word statistics");
    let text = embedded_text();
    print_report(text.as_str());
    let args = std::process::args();
    if args.len() > 0 {
        let path = args[0].clone();
        println!("file: {}", path);
        match std::fs::read_to_string(&path) {
            Ok(file_text) => print_report(file_text.as_str()),
            Err(error) => println!("file read error: {}", error.message()),
        }
    }
}
