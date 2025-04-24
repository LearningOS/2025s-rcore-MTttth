//! banker
use alloc::collections::BTreeMap;
/// Banker
pub struct Banker {
    available: BTreeMap<usize, isize>,
    allocation: BTreeMap<usize, BTreeMap<usize, isize>>,
    need: BTreeMap<usize, BTreeMap<usize, isize>>,
}

impl Banker {
    /// new a Banker
    pub fn new() -> Self {
        Self {
            available: BTreeMap::new(),
            allocation: BTreeMap::new(),
            need: BTreeMap::new(),
        }
    }
    /// Register new Resources
    pub fn register_resource(&mut self, id: usize, total: usize) {
        self.available.insert(id, total as isize);
    }
    /// Register new tasks
    pub fn register_thread(&mut self, tid: usize) {
        self.allocation.insert(tid, BTreeMap::new());
        self.need.insert(tid, BTreeMap::new());
    }
    /// Set task need
    pub fn increase_need(&mut self, tid: usize, id: usize, need: usize) {
        let need_map = self.need.entry(tid).or_insert_with(BTreeMap::new);
        let entry = need_map.entry(id).or_insert(0);
        *entry += need as isize;
    }
    /// judge safe
    pub fn is_safe(&self) -> bool {
        let mut work = self.available.clone();
        let mut finish = BTreeMap::new();
        for tid in self.allocation.keys() {
            finish.insert(*tid, false);
        }
        loop {
            let mut found = false;
            for (&tid, need_map) in &self.need {
                if finish[&tid] {
                    continue;
                }
                let mut can_finish = true;
                for (&id, &need) in need_map {
                    let available = work.get(&id).copied().unwrap_or(0);
                    if need > available {
                        can_finish = false;
                        break;
                    }
                }
                if can_finish {
                    if let Some(alloc_map) = self.allocation.get(&tid) {
                        for (&id, &alloc) in alloc_map {
                            *work.entry(id).or_insert(0) += alloc;
                        }
                    }
                    finish.insert(tid, true);
                    found = true;
                }
            }
            if found == false {
                break;
            }
        }
        finish.values().all(|&f| f)
    }
    /// try to alloc
    pub fn try_allocate(&mut self, tid: usize, id: usize, amount: usize) -> bool {
        let safe = self.is_safe();
        if safe {
            self.allocation
                .get_mut(&tid)
                .unwrap()
                .entry(id)
                .and_modify(|e| *e += amount as isize)
                .or_insert(amount as isize);
            *self.available.get_mut(&id).unwrap() -= amount as isize;
            self.need
                .get_mut(&tid)
                .unwrap()
                .entry(id)
                .and_modify(|e| *e -= amount as isize);
        }
        safe
    }
    /// increase_available
    pub fn increase_available(&mut self, id: usize, increase: usize) {
        let available = self.available.entry(id).or_insert(0);
        *available += increase as isize;
    }
}
