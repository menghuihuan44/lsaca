// audit: importing a private trait from another module is rejected (E0603).
mod helper;
use helper::Hidden;

struct Local {
    value: i64,
}

impl Hidden for Local {
    fn id(&self) -> i64 {
        self.value
    }
}

fn main() {
}
