// string_split: build a comma-separated string of 50,000 decimal numbers
// (about 194,500 bytes), split it, parse every field and sum it.
const TOKENS = 50000;

function main() {
  const parts = [];
  for (let i = 0; i < TOKENS; i++) {
    parts.push(String(i % 1000) + ",");
  }
  const text = parts.join("");

  let count = 0;
  let sum = 0;
  for (const part of text.split(",")) {
    if (part.length > 0) {
      sum += Number(part);
      count++;
    }
  }
  console.log(`checksum=${text.length},${count},${sum}`);
}

main();
