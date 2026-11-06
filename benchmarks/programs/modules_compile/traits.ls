// traits: user-defined traits, per-type implementations and dispatch.
// Part of the modules_compile compile-time workload.
//
// Every trait is implemented for several user types and called through
// concrete receivers; there are no trait objects and no trait bounds on
// generic functions. Inherent methods shadow trait methods where both define
// the same name.

pub const MODULUS: i64 = 1000000007;

pub trait Shape {
    fn area(&self) -> i64;

    fn perimeter(&self) -> i64;

    fn name(&self) -> String;
}

pub trait Scored {
    fn score(&self) -> i64;

    fn grade(&self) -> String;
}

pub trait Labeled {
    fn label(&self) -> String;
}

pub trait Resettable {
    fn reset(&mut self);
}

pub struct Rectangle {
    pub width: i64,
    pub height: i64,
}

impl Rectangle {
    pub fn new(width: i64, height: i64) -> Rectangle {
        Rectangle {
            width: width,
            height: height,
        }
    }
}

impl Shape for Rectangle {
    fn area(&self) -> i64 {
        self.width * self.height
    }

    fn perimeter(&self) -> i64 {
        2 * (self.width + self.height)
    }

    fn name(&self) -> String {
        String::from("rectangle")
    }
}

pub struct Circle {
    pub radius: i64,
}

impl Circle {
    pub fn new(radius: i64) -> Circle {
        Circle { radius: radius }
    }
}

impl Shape for Circle {
    fn area(&self) -> i64 {
        3 * self.radius * self.radius
    }

    fn perimeter(&self) -> i64 {
        6 * self.radius
    }

    fn name(&self) -> String {
        String::from("circle")
    }
}

pub struct Square {
    pub side: i64,
}

impl Square {
    pub fn new(side: i64) -> Square {
        Square { side: side }
    }

    pub fn area(&self) -> i64 {
        self.side * self.side
    }

    pub fn double(&self) -> Square {
        Square::new(self.side * 2)
    }
}

impl Shape for Square {
    fn area(&self) -> i64 {
        -1
    }

    fn perimeter(&self) -> i64 {
        4 * self.side
    }

    fn name(&self) -> String {
        String::from("square")
    }
}

pub struct Triangle {
    pub a: i64,
    pub b: i64,
    pub c: i64,
}

impl Triangle {
    pub fn new(a: i64, b: i64, c: i64) -> Triangle {
        Triangle { a: a, b: b, c: c }
    }
}

impl Shape for Triangle {
    fn area(&self) -> i64 {
        (self.a * self.b) / 2
    }

    fn perimeter(&self) -> i64 {
        self.a + self.b + self.c
    }

    fn name(&self) -> String {
        String::from("triangle")
    }
}

pub struct Record {
    pub passed: i64,
    pub failed: i64,
    pub weight: i64,
}

impl Record {
    pub fn new(passed: i64, failed: i64, weight: i64) -> Record {
        Record {
            passed: passed,
            failed: failed,
            weight: weight,
        }
    }

    pub fn attempts(&self) -> i64 {
        self.passed + self.failed
    }
}

impl Scored for Record {
    fn score(&self) -> i64 {
        let attempts = self.attempts();
        if attempts == 0 {
            return 0;
        }
        self.passed * 100 / attempts * self.weight / 10
    }

    fn grade(&self) -> String {
        let score = self.score();
        if score >= 90 {
            String::from("A")
        } else if score >= 80 {
            String::from("B")
        } else if score >= 70 {
            String::from("C")
        } else {
            String::from("D")
        }
    }
}

pub struct Review {
    pub stars: i64,
}

impl Review {
    pub fn new(stars: i64) -> Review {
        Review { stars: stars }
    }
}

impl Scored for Review {
    fn score(&self) -> i64 {
        self.stars * 20
    }

    fn grade(&self) -> String {
        if self.stars >= 4 {
            String::from("good")
        } else {
            String::from("weak")
        }
    }
}

pub struct Ticket {
    pub id: i64,
    pub title: String,
}

impl Ticket {
    pub fn new(id: i64, title: String) -> Ticket {
        Ticket { id: id, title: title }
    }
}

impl Labeled for Ticket {
    fn label(&self) -> String {
        format!("ticket-{}-{}", self.id, self.title)
    }
}

pub struct Version {
    pub major: i64,
    pub minor: i64,
}

impl Version {
    pub fn new(major: i64, minor: i64) -> Version {
        Version {
            major: major,
            minor: minor,
        }
    }
}

impl Labeled for Version {
    fn label(&self) -> String {
        format!("v{}.{}", self.major, self.minor)
    }
}

pub struct Tally {
    pub value: i64,
    pub resets: i64,
}

impl Tally {
    pub fn new(value: i64) -> Tally {
        Tally {
            value: value,
            resets: 0,
        }
    }

    pub fn add(&mut self, amount: i64) {
        self.value += amount;
    }
}

impl Resettable for Tally {
    fn reset(&mut self) {
        self.value = 0;
        self.resets += 1;
    }
}

pub struct Counter {
    pub value: i64,
}

impl Counter {
    pub fn new(value: i64) -> Counter {
        Counter { value: value }
    }
}

impl Resettable for Counter {
    fn reset(&mut self) {
        self.value = 0;
    }
}

pub trait Describe<T> {
    fn describe(&self, extra: T) -> String;
}

pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub fn new(x: i64, y: i64) -> Point {
        Point { x: x, y: y }
    }
}

impl Describe<i64> for Point {
    fn describe(&self, extra: i64) -> String {
        format!("point({}, {}) + {}", self.x, self.y, extra)
    }
}

pub struct Tag {
    pub label: String,
}

impl Tag {
    pub fn new(label: String) -> Tag {
        Tag { label: label }
    }
}

impl Describe<String> for Tag {
    fn describe(&self, extra: String) -> String {
        self.label + ":" + &extra
    }
}

pub fn shape_checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;

    let rectangle = Rectangle::new(3, 4);
    acc = (acc * 31 + rectangle.area()) % MODULUS;
    acc = (acc * 31 + rectangle.perimeter()) % MODULUS;
    acc = (acc * 31 + rectangle.name().len() as i64) % MODULUS;

    let circle = Circle::new(5);
    acc = (acc * 31 + circle.area()) % MODULUS;
    acc = (acc * 31 + circle.perimeter()) % MODULUS;
    acc = (acc * 31 + circle.name().len() as i64) % MODULUS;

    let square = Square::new(6);
    acc = (acc * 31 + square.area()) % MODULUS;
    acc = (acc * 31 + square.perimeter()) % MODULUS;
    acc = (acc * 31 + square.double().area()) % MODULUS;
    acc = (acc * 31 + square.name().len() as i64) % MODULUS;

    let triangle = Triangle::new(4, 5, 6);
    acc = (acc * 31 + triangle.area()) % MODULUS;
    acc = (acc * 31 + triangle.perimeter()) % MODULUS;

    acc % MODULUS
}

pub fn score_checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;

    let record = Record::new(7, 3, 10);
    acc = (acc * 31 + record.attempts()) % MODULUS;
    acc = (acc * 31 + record.score()) % MODULUS;
    acc = (acc * 31 + record.grade().len() as i64) % MODULUS;

    let perfect = Record::new(9, 0, 10);
    acc = (acc * 31 + perfect.score()) % MODULUS;

    let empty = Record::new(0, 0, 10);
    acc = (acc * 31 + empty.score()) % MODULUS;

    let review = Review::new(4);
    acc = (acc * 31 + review.score()) % MODULUS;
    acc = (acc * 31 + review.grade().len() as i64) % MODULUS;

    let weak = Review::new(2);
    acc = (acc * 31 + weak.score()) % MODULUS;

    acc % MODULUS
}

pub fn label_checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;

    let ticket = Ticket::new(7, String::from("printer"));
    let ticket_label = ticket.label();
    acc = (acc * 31 + ticket_label.len() as i64) % MODULUS;

    let version = Version::new(2, 11);
    let version_label = version.label();
    acc = (acc * 31 + version_label.len() as i64) % MODULUS;
    acc = (acc * 31 + version.major) % MODULUS;

    acc % MODULUS
}

pub fn reset_checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;

    let mut tally = Tally::new(10);
    tally.add(5);
    acc = (acc * 31 + tally.value) % MODULUS;
    tally.reset();
    acc = (acc * 31 + tally.value) % MODULUS;
    acc = (acc * 31 + tally.resets) % MODULUS;
    tally.add(7);
    acc = (acc * 31 + tally.value) % MODULUS;
    tally.reset();
    acc = (acc * 31 + tally.resets) % MODULUS;

    let mut counter = Counter::new(9);
    counter.reset();
    acc = (acc * 31 + counter.value) % MODULUS;

    acc % MODULUS
}

pub fn describe_checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;

    let point = Point::new(1, 2);
    let point_text = point.describe(3);
    acc = (acc * 31 + point_text.len() as i64) % MODULUS;

    let tag = Tag::new(String::from("t"));
    let tag_text = tag.describe(String::from("v"));
    acc = (acc * 31 + tag_text.len() as i64) % MODULUS;

    acc % MODULUS
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;
    acc = (acc * 31 + shape_checksum(seed)) % MODULUS;
    acc = (acc * 31 + score_checksum(seed)) % MODULUS;
    acc = (acc * 31 + label_checksum(seed)) % MODULUS;
    acc = (acc * 31 + reset_checksum(seed)) % MODULUS;
    acc = (acc * 31 + describe_checksum(seed)) % MODULUS;
    acc % MODULUS
}
