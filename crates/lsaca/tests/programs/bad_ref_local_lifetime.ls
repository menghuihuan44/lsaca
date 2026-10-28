fn main() {
    let r = {
        let x = 1;
        &x
    };
    let y = *r;
}
