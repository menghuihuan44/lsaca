mod shapes;
use shapes::{area, origin, Point};

fn main() {
    let point = Point { x: 3, y: 4 };
    println!("{} {}", point.x, point.y);
    println!("{}", area(point));
    let base = origin();
    println!("{} {}", base.x, base.y);
}
