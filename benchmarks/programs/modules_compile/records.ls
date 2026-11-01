// records: structs with owned fields, nested records, enums and collections of
// records. Part of the modules_compile compile-time workload.
//
// All aggregation methods read fields through a shared reference and use index
// loops; transformations that would rewrite fields build a fresh registry
// instead, which keeps the borrow structure simple.

pub const MODULUS: i64 = 1000000007;

pub enum Department {
    Engineering,
    Sales,
    Support,
    Finance,
}

pub fn department_name(department: Department) -> String {
    match department {
        Department::Engineering => String::from("engineering"),
        Department::Sales => String::from("sales"),
        Department::Support => String::from("support"),
        Department::Finance => String::from("finance"),
    }
}

pub fn department_weight(department: Department) -> i64 {
    match department {
        Department::Engineering => 3,
        Department::Sales => 2,
        Department::Support => 1,
        Department::Finance => 4,
    }
}

pub enum Status {
    Active,
    Paused,
    Closed(String),
}

pub fn status_code(status: Status) -> i64 {
    match status {
        Status::Active => 1,
        Status::Paused => 2,
        Status::Closed(reason) => 3 + reason.len(),
    }
}

pub fn status_label(status: Status) -> String {
    match status {
        Status::Active => String::from("active"),
        Status::Paused => String::from("paused"),
        Status::Closed(reason) => String::from("closed") + reason,
    }
}

pub struct Employee {
    pub name: String,
    pub department: Department,
    pub salary: i64,
    pub years: i64,
}

impl Employee {
    pub fn new(name: String, department: Department, salary: i64, years: i64) -> Employee {
        Employee {
            name: name,
            department: department,
            salary: salary,
            years: years,
        }
    }

    pub fn level(&self) -> i64 {
        let base = department_weight(self.department);
        base * 10 + self.years
    }

    pub fn cost(&self) -> i64 {
        self.salary + self.years * 100
    }

    pub fn summary(&self) -> String {
        format!(
            "{}:{}:{},{}",
            self.name,
            department_name(self.department),
            self.salary,
            self.years
        )
    }
}

pub struct Address {
    pub city: String,
    pub zip: i64,
}

impl Address {
    pub fn new(city: String, zip: i64) -> Address {
        Address { city: city, zip: zip }
    }

    pub fn city_length(&self) -> i64 {
        self.city.len()
    }

    pub fn postal_checksum(&self) -> i64 {
        (self.zip * 31 + self.city_length()) % MODULUS
    }
}

pub struct Customer {
    pub name: String,
    pub address: Address,
    pub orders: i64,
}

impl Customer {
    pub fn new(name: String, address: Address, orders: i64) -> Customer {
        Customer {
            name: name,
            address: address,
            orders: orders,
        }
    }

    pub fn weight(&self) -> i64 {
        self.name.len() * 10 + self.address.city_length() + self.orders
    }

    pub fn describe(&self) -> String {
        format!(
            "{} of {} ({} orders)",
            self.name, self.address.city, self.orders
        )
    }
}

pub struct Registry {
    employees: Vec<Employee>,
    total_added: i64,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            employees: Vec::new(),
            total_added: 0,
        }
    }

    pub fn add(&mut self, employee: Employee) {
        self.employees.push(employee);
        self.total_added += 1;
    }

    pub fn len(&self) -> i64 {
        self.employees.len()
    }

    pub fn total_added(&self) -> i64 {
        self.total_added
    }

    pub fn total_salary(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.employees.len() {
            total += self.employees[index].salary;
            index += 1;
        }
        total
    }

    pub fn total_cost(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.employees.len() {
            total += self.employees[index].cost();
            index += 1;
        }
        total
    }

    pub fn total_years(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.employees.len() {
            total += self.employees[index].years;
            index += 1;
        }
        total
    }

    pub fn average_salary(&self) -> i64 {
        if self.employees.len() == 0 {
            return 0;
        }
        self.total_salary() / self.employees.len()
    }

    pub fn highest_salary(&self) -> i64 {
        let mut best = 0;
        let mut index = 0;
        while index < self.employees.len() {
            let salary = self.employees[index].salary;
            if salary > best {
                best = salary;
            }
            index += 1;
        }
        best
    }

    pub fn best_paid_name(&self) -> String {
        if self.employees.len() == 0 {
            return String::from("");
        }
        let mut best = 0;
        let mut index = 1;
        while index < self.employees.len() {
            if self.employees[index].salary > self.employees[best].salary {
                best = index;
            }
            index += 1;
        }
        format!("{}", self.employees[best].name)
    }

    pub fn count_department(&self, department: Department) -> i64 {
        let mut count = 0;
        let mut index = 0;
        while index < self.employees.len() {
            let current = self.employees[index].department;
            let matched = match current {
                Department::Engineering => department_weight(department) == 3,
                Department::Sales => department_weight(department) == 2,
                Department::Support => department_weight(department) == 1,
                Department::Finance => department_weight(department) == 4,
            };
            if matched {
                count += 1;
            }
            index += 1;
        }
        count
    }

    pub fn highest_level(&self) -> i64 {
        let mut best = 0;
        let mut index = 0;
        while index < self.employees.len() {
            let level = self.employees[index].level();
            if level > best {
                best = level;
            }
            index += 1;
        }
        best
    }

    pub fn find_by_name(&self, wanted: &String) -> i64 {
        let wanted_text = format!("{}", wanted);
        let mut index = 0;
        while index < self.employees.len() {
            let current = format!("{}", self.employees[index].name);
            if current == wanted_text {
                return index;
            }
            index += 1;
        }
        return -1;
    }

    pub fn department_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::with_capacity(self.employees.len());
        let mut index = 0;
        while index < self.employees.len() {
            names.push(department_name(self.employees[index].department));
            index += 1;
        }
        names
    }

    pub fn salaries(&self) -> Vec<i64> {
        let mut values: Vec<i64> = Vec::with_capacity(self.employees.len());
        let mut index = 0;
        while index < self.employees.len() {
            values.push(self.employees[index].salary);
            index += 1;
        }
        values
    }

    pub fn with_raise(&self, amount: i64) -> Registry {
        let mut fresh = Registry::new();
        let mut index = 0;
        while index < self.employees.len() {
            let name = format!("{}", self.employees[index].name);
            let department = self.employees[index].department;
            let salary = self.employees[index].salary + amount;
            let years = self.employees[index].years;
            fresh.add(Employee {
                name: name,
                department: department,
                salary: salary,
                years: years,
            });
            index += 1;
        }
        fresh
    }

    pub fn rotated(&self, offset: i64) -> Registry {
        let mut fresh = Registry::new();
        if self.employees.len() == 0 {
            return fresh;
        }
        let mut step = 0;
        while step < self.employees.len() {
            let index = (step + offset) % self.employees.len();
            let name = format!("{}", self.employees[index].name);
            let department = self.employees[index].department;
            let salary = self.employees[index].salary;
            let years = self.employees[index].years;
            fresh.add(Employee {
                name: name,
                department: department,
                salary: salary,
                years: years,
            });
            step += 1;
        }
        fresh
    }

    pub fn checksum(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.employees.len() {
            let employee = &self.employees[index];
            total = (total * 31 + employee.salary) % MODULUS;
            total = (total * 31 + department_weight(employee.department)) % MODULUS;
            total = (total * 31 + employee.name.len()) % MODULUS;
            index += 1;
        }
        total
    }
}

pub struct Review {
    pub rating: i64,
    pub comment: String,
}

impl Review {
    pub fn new(rating: i64, comment: String) -> Review {
        Review {
            rating: rating,
            comment: comment,
        }
    }

    pub fn weight(&self) -> i64 {
        let mut weight = self.rating * 10;
        if self.comment.len() > 5 {
            weight += 5;
        }
        weight
    }

    pub fn summary(&self) -> String {
        format!("{}/10 {}", self.rating, self.comment)
    }
}

pub struct ReviewBoard {
    reviews: Vec<Review>,
}

impl ReviewBoard {
    pub fn new() -> ReviewBoard {
        ReviewBoard {
            reviews: Vec::new(),
        }
    }

    pub fn add(&mut self, review: Review) {
        self.reviews.push(review);
    }

    pub fn average_rating(&self) -> i64 {
        if self.reviews.len() == 0 {
            return 0;
        }
        let mut total = 0;
        let mut index = 0;
        while index < self.reviews.len() {
            total += self.reviews[index].rating;
            index += 1;
        }
        total / self.reviews.len()
    }

    pub fn total_weight(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.reviews.len() {
            total += self.reviews[index].weight();
            index += 1;
        }
        total
    }

    pub fn comments_length(&self) -> i64 {
        let mut total = 0;
        let mut index = 0;
        while index < self.reviews.len() {
            total += self.reviews[index].comment.len();
            index += 1;
        }
        total
    }
}

pub fn checksum(seed: i64) -> i64 {
    let mut acc: i64 = seed % MODULUS;

    let mut registry = Registry::new();
    registry.add(Employee::new(
        String::from("ada"),
        Department::Engineering,
        120,
        7,
    ));
    registry.add(Employee::new(String::from("bob"), Department::Sales, 90, 3));
    registry.add(Employee::new(
        String::from("cy"),
        Department::Engineering,
        150,
        9,
    ));
    registry.add(Employee::new(
        String::from("di"),
        Department::Support,
        70,
        2,
    ));

    acc = (acc * 31 + registry.len()) % MODULUS;
    acc = (acc * 31 + registry.total_salary()) % MODULUS;
    acc = (acc * 31 + registry.average_salary()) % MODULUS;
    acc = (acc * 31 + registry.highest_salary()) % MODULUS;
    acc = (acc * 31 + registry.best_paid_name().len()) % MODULUS;
    acc = (acc * 31 + registry.count_department(Department::Engineering)) % MODULUS;
    acc = (acc * 31 + registry.highest_level()) % MODULUS;
    acc = (acc * 31 + registry.total_cost()) % MODULUS;
    acc = (acc * 31 + registry.total_years()) % MODULUS;
    acc = (acc * 31 + registry.checksum()) % MODULUS;

    let raised = registry.with_raise(10);
    acc = (acc * 31 + raised.total_salary()) % MODULUS;

    let rotated = registry.rotated(2);
    acc = (acc * 31 + rotated.checksum()) % MODULUS;

    let wanted = String::from("cy");
    acc = (acc * 31 + registry.find_by_name(&wanted)) % MODULUS;

    let names = registry.department_names();
    acc = (acc * 31 + names.len()) % MODULUS;

    let status = Status::Closed(String::from("moved"));
    acc = (acc * 31 + status_code(status)) % MODULUS;

    let active = Status::Active;
    acc = (acc * 31 + status_code(active)) % MODULUS;

    let customer = Customer::new(
        String::from("erin"),
        Address::new(String::from("lisbon"), 1100),
        4,
    );
    acc = (acc * 31 + customer.weight()) % MODULUS;
    acc = (acc * 31 + customer.address.postal_checksum()) % MODULUS;
    acc = (acc * 31 + customer.describe().len()) % MODULUS;

    let mut board = ReviewBoard::new();
    board.add(Review::new(8, String::from("solid")));
    board.add(Review::new(5, String::from("uneven corners")));
    acc = (acc * 31 + board.average_rating()) % MODULUS;
    acc = (acc * 31 + board.total_weight()) % MODULUS;
    acc = (acc * 31 + board.comments_length()) % MODULUS;

    acc % MODULUS
}
