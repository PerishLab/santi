use std::{collections::HashMap, sync::Mutex};

pub(super) struct Handoff<'a> {
    held: &'a Mutex<HashMap<String, usize>>,
    stamp: String,
}

impl<'a> Handoff<'a> {
    pub fn new(held: &'a Mutex<HashMap<String, usize>>, stamp: &str) -> Self {
        *held.lock().unwrap().entry(stamp.to_string()).or_default() += 1;
        Self {
            held,
            stamp: stamp.to_string(),
        }
    }
}

impl Drop for Handoff<'_> {
    fn drop(&mut self) {
        let mut held = self.held.lock().unwrap();
        let count = held.get_mut(&self.stamp).expect("retained handoff");
        *count -= 1;
        if *count == 0 {
            held.remove(&self.stamp);
        }
    }
}
