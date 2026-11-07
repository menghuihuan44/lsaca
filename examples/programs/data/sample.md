# Sample document

This file is read through the optional file argument. Inline styles:
**bold**, *italic*, `code`, and a [link](https://example.com/sample?x=1&y=2).
Backslash escapes: \*kept literal\*, \`kept literal\`, \# kept literal.

## Paragraphs and lists

The next paragraph is split over three source lines and must be joined
with single spaces; the paragraph should also preserve unicode text such
as café, naïve and 日本語.

- alpha item
- beta item with **bold words**
- gamma item with a [link](https://example.com/gamma)

> A block quote that spans two lines,
> with *emphasis* and `inline code`.

```lsaca
fn main() {
    let total = 1 + 2;
    println!("total = {} < 4 && 4 > 3", total);
}
```

#hashtag is not a heading; 5 < 6 & 7 > 6 must be escaped.

### Third level heading

A final paragraph.
