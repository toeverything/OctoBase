use std::{mem, ops::Range};

#[derive(Debug, PartialEq, Eq)]
pub enum OrderRange {
    Range(Range<u64>),
    Fragment(Vec<Range<u64>>),
}

impl Default for OrderRange {
    fn default() -> Self {
        Self::Range(0..0)
    }
}

impl From<Range<u64>> for OrderRange {
    fn from(range: Range<u64>) -> Self {
        Self::Range(range)
    }
}

impl From<Vec<Range<u64>>> for OrderRange {
    fn from(value: Vec<Range<u64>>) -> Self {
        Self::Fragment(value)
    }
}

fn is_continuous_range(lhs: &Range<u64>, rhs: &Range<u64>) -> bool {
    lhs.end >= rhs.start && lhs.start <= rhs.end
}

impl OrderRange {
    pub fn is_empty(&self) -> bool {
        match self {
            OrderRange::Range(range) => range.is_empty(),
            OrderRange::Fragment(vec) => vec.is_empty(),
        }
    }

    pub fn extends<T>(&mut self, list: T)
    where
        T: Into<Vec<Range<u64>>>,
    {
        let list: Vec<_> = list.into();
        if list.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = OrderRange::Fragment(list);
        } else {
            match self {
                OrderRange::Range(old) => {
                    let old = old.clone();
                    // swap and push is faster then push one by one
                    *self = OrderRange::Fragment(list);
                    self.push(old);
                }
                OrderRange::Fragment(ranges) => {
                    ranges.extend(list);
                }
            }
        }

        self.sort();
        self.squash();
    }

    /// Push new range to current one.
    /// Range will be merged if overlap exists or turned into fragment if it's not continuous.
    pub fn push(&mut self, range: Range<u64>) {
        match self {
            OrderRange::Range(r) => {
                if r.start == r.end {
                    *self = range.into();
                } else if is_continuous_range(r, &range) {
                    r.end = r.end.max(range.end);
                    r.start = r.start.min(range.start);
                } else {
                    *self = OrderRange::Fragment(if r.start < range.start {
                        vec![r.clone(), range]
                    } else {
                        vec![range, r.clone()]
                    });
                }
            }
            OrderRange::Fragment(ranges) => {
                if ranges.is_empty() {
                    *self = OrderRange::Range(range);
                } else {
                    let search_result = ranges.binary_search_by(|r| {
                        if is_continuous_range(r, &range) {
                            std::cmp::Ordering::Equal
                        } else if r.end < range.start {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    });

                    match search_result {
                        Ok(idx) => {
                            let old = &mut ranges[idx];
                            ranges[idx] = old.start.min(range.start)..old.end.max(range.end);
                            self.squash();
                        }
                        Err(idx) => ranges.insert(idx, range),
                    }
                }
            }
        }
    }

    pub fn merge(&mut self, other: Self) {
        let raw = std::mem::take(self);
        if raw.is_empty() {
            *self = other;
            return;
        }
        *self = match (raw, other) {
            (OrderRange::Range(a), OrderRange::Range(b)) => OrderRange::Fragment(vec![a, b]),
            (OrderRange::Fragment(mut a), OrderRange::Range(b)) => {
                a.push(b);
                OrderRange::Fragment(a)
            }
            (OrderRange::Range(a), OrderRange::Fragment(b)) => {
                let mut v = b;
                v.push(a);
                OrderRange::Fragment(v)
            }
            (OrderRange::Fragment(mut a), OrderRange::Fragment(mut b)) => {
                a.append(&mut b);
                OrderRange::Fragment(a)
            }
        };

        self.sort();
        self.squash()
    }

    /// Merge all available ranges list into one.
    fn squash(&mut self) {
        // merge all available ranges
        if let OrderRange::Fragment(ranges) = self {
            if ranges.is_empty() {
                *self = OrderRange::Range(0..0);
                return;
            }

            let mut cur_idx = 0;
            let mut next_idx = 1;
            while next_idx < ranges.len() {
                let cur = &ranges[cur_idx];
                let next = &ranges[next_idx];
                if is_continuous_range(cur, next) {
                    ranges[cur_idx] = cur.start.min(next.start)..cur.end.max(next.end);
                    ranges[next_idx] = 0..0;
                } else {
                    cur_idx = next_idx;
                }

                next_idx += 1;
            }

            ranges.retain(|r| !r.is_empty());
            if ranges.len() == 1 {
                *self = OrderRange::Range(ranges[0].clone());
            }
        }
    }

    fn sort(&mut self) {
        if let OrderRange::Fragment(ranges) = self {
            ranges.sort_by(|lhs, rhs| lhs.start.cmp(&rhs.start));
        }
    }

    pub fn pop(&mut self) -> Option<Range<u64>> {
        if self.is_empty() {
            None
        } else {
            match self {
                OrderRange::Range(range) => Some(mem::replace(range, 0..0)),
                OrderRange::Fragment(list) => Some(list.remove(0)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrderRange;
    #[test]
    fn test_range_push() {
        let mut range: OrderRange = (0..10).into();

        range.push(5..15);
        assert_eq!(range, (0..15).into());

        // turn to fragment
        range.push(20..30);
        assert_eq!(range, OrderRange::Fragment(vec![(0..15), (20..30)]));

        // auto merge
        range.push(15..16);
        assert_eq!(range, OrderRange::Fragment(vec![(0..16), (20..30)]));

        // squash
        range.push(16..20);
        assert_eq!(range, OrderRange::Range(0..30));
    }

    #[test]
    fn test_ranges_squash() {
        let mut range = OrderRange::Fragment(vec![(0..10), (20..30)]);

        // do nothing
        range.squash();
        assert_eq!(range, OrderRange::Fragment(vec![(0..10), (20..30)]));

        // merged into list
        range = OrderRange::Fragment(vec![(0..10), (10..20), (30..40)]);
        range.squash();
        assert_eq!(range, OrderRange::Fragment(vec![(0..20), (30..40)]));

        // turn to range
        range = OrderRange::Fragment(vec![(0..10), (10..20), (20..30)]);
        range.squash();
        assert_eq!(range, OrderRange::Range(0..30));
    }
}
