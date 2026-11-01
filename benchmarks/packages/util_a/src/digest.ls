// digest: rolling hash helpers for the util_a library package.

pub const MODULUS: i64 = 1000000007;
pub const MULTIPLIER: i64 = 131;

pub struct Mixer {
    pub state: i64,
    pub steps: i64,
}

impl Mixer {
    pub fn new(seed: i64) -> Mixer {
        let mut initial = seed % MODULUS;
        if initial < 0 {
            initial = initial + MODULUS;
        }
        return Mixer { state: initial, steps: 0 };
    }

    pub fn absorb(&mut self, value: i64) {
        let normalized = normalize(value);
        self.state = (self.state * MULTIPLIER + normalized) % MODULUS;
        self.steps = self.steps + 1;
    }

    pub fn multiply(&mut self, factor: i64) {
        let normalized = normalize(factor);
        self.state = (self.state * (normalized + 1)) % MODULUS;
        self.steps = self.steps + 1;
    }

    pub fn finish(&self) -> i64 {
        return (self.state * 31 + self.steps) % MODULUS;
    }
}

pub fn normalize(value: i64) -> i64 {
    let mut result = value % MODULUS;
    if result < 0 {
        result = result + MODULUS;
    }
    return result;
}

pub fn mix(acc: i64, value: i64) -> i64 {
    return (normalize(acc) * MULTIPLIER + normalize(value)) % MODULUS;
}

pub fn hash_text(text: &String) -> i64 {
    let mut state = 7 % MODULUS;
    for ch in text.chars() {
        state = (state * MULTIPLIER + (ch as i64)) % MODULUS;
    }
    return state;
}

pub fn fold_words(words: Vec<String>) -> i64 {
    let mut state = 13 % MODULUS;
    for word in words {
        state = (state * MULTIPLIER + hash_text(&word)) % MODULUS;
    }
    return state;
}

pub fn checksum(seed: i64) -> i64 {
    let mut mixer = Mixer::new(seed);
    mixer.absorb(11);
    mixer.absorb(29);
    mixer.multiply(37);
    let mut acc = mixer.finish();
    let text = String::from("module digest");
    acc = (acc * 31 + hash_text(&text)) % MODULUS;
    let mut words: Vec<String> = Vec::new();
    words.push(String::from("alpha"));
    words.push(String::from("beta"));
    words.push(String::from("gamma"));
    acc = (acc * 31 + fold_words(words)) % MODULUS;
    acc = mix(acc, 99);
    return acc;
}
