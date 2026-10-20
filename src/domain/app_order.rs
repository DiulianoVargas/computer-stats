//! Row order in the Processes View: by CPU or memory, frozen while the pointer is over the list
//! (003 FR-008, FR-010; research R9).

use std::cmp::Ordering;

use super::apps::{AppKey, AppUsage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Cpu,
    Memory,
}

/// Indices into `apps`, in display order. Unfrozen: by `sort`, highest first, ties by name
/// (case-insensitive). Frozen: Apps keep the order of `frozen`; exited ones drop out and new ones
/// are appended, sorted among themselves.
pub fn order(apps: &[AppUsage], sort: Sort, frozen: Option<&[AppKey]>) -> Vec<usize> {
    let mut sorted: Vec<usize> = (0..apps.len()).collect();
    sorted.sort_by(|&a, &b| compare(&apps[a], &apps[b], sort));
    let Some(previous) = frozen else {
        return sorted;
    };
    let mut result: Vec<usize> = previous
        .iter()
        .filter_map(|key| apps.iter().position(|a| &a.key == key))
        .collect();
    for i in sorted {
        if !result.contains(&i) {
            result.push(i);
        }
    }
    result
}

fn compare(a: &AppUsage, b: &AppUsage, sort: Sort) -> Ordering {
    let by_column = match sort {
        Sort::Cpu => b
            .cpu_pct
            .unwrap_or(0.0)
            .total_cmp(&a.cpu_pct.unwrap_or(0.0)),
        Sort::Memory => b.private_bytes.cmp(&a.private_bytes),
    };
    by_column.then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, cpu: Option<f32>, mem: u64) -> AppUsage {
        AppUsage {
            key: name.to_lowercase(),
            name: name.into(),
            path: None,
            processes: Vec::new(),
            cpu_pct: cpu,
            private_bytes: mem,
        }
    }

    fn names(apps: &[AppUsage], order: &[usize]) -> Vec<String> {
        order.iter().map(|&i| apps[i].name.clone()).collect()
    }

    #[test]
    fn by_cpu_then_name() {
        let apps = [
            app("b", Some(5.0), 1),
            app("A", Some(5.0), 9),
            app("c", Some(20.0), 0),
            app("d", None, 99),
        ];
        assert_eq!(
            names(&apps, &order(&apps, Sort::Cpu, None)),
            ["c", "A", "b", "d"]
        );
    }

    #[test]
    fn by_memory() {
        let apps = [
            app("a", Some(50.0), 10),
            app("b", Some(1.0), 30),
            app("c", None, 20),
        ];
        assert_eq!(
            names(&apps, &order(&apps, Sort::Memory, None)),
            ["b", "c", "a"]
        );
    }

    #[test]
    fn frozen_keeps_order_drops_exited_appends_new() {
        let apps = [
            app("new", Some(90.0), 0),
            app("x", Some(1.0), 0),
            app("y", Some(50.0), 0),
        ];
        let previous: Vec<AppKey> = vec!["x".into(), "gone".into(), "y".into()];
        let o = order(&apps, Sort::Cpu, Some(&previous));
        assert_eq!(names(&apps, &o), ["x", "y", "new"]);
    }
}
