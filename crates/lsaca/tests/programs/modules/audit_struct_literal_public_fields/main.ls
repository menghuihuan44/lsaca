// audit: pub fields allow a struct literal outside the defining module.
mod point;

fn main() {
    let point = point::Point { x: 3, y: 4 };
    println!("{} {}", point.x, point.y);
}
