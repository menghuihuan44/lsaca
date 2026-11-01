mod util;

fn main() {
    let scaled = util::SCALE * util::math::double(3);
    println!("{}", scaled);
}
