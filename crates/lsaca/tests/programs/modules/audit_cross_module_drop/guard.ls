pub struct Guard {
    pub id: i64,
}

impl Drop for Guard {
    fn drop(&mut self) {
        println!("drop {}", self.id);
    }
}

pub fn make(id: i64) -> Guard {
    Guard { id: id }
}
