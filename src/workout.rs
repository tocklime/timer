use csv;
use serde::Deserialize;

enum Work {
    Seconds(u32),
    Composite(Vec<WorkoutItem>),
}

pub struct WorkoutItem {
    name: String,
    reps: u32,
    rest_between: u32,
    content: Work,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct FlatStatus {
    pub name: String,
    pub this_rep: u32,
    pub total_reps: u32,
    pub duration: u32,
    pub absolute_start_time: u32,
}

impl FlatStatus {
    pub fn from_csv(csv_str: &str) -> Result<Vec<FlatStatus>, csv::Error> {
        let reader = csv::Reader::from_reader(csv_str.as_bytes());
        let mut ans = Vec::new();
        for r in reader.deserialize() {
            let r: FlatStatus = r?;
            ans.push(r);
        }
        Ok(ans)
    }
}

impl WorkoutItem {
    fn total_duration(&self) -> u32 {
        let one_work = match &self.content {
            Work::Seconds(x) => *x,
            Work::Composite(v) => v.iter().map(|wi| wi.total_duration()).sum(),
        };
        one_work * self.reps + (self.rest_between * (self.reps - 1))
    }
    pub fn describe(&self) -> Vec<FlatStatus> {
        let mut ans = Vec::new();
        let mut start = 0;
        let mut add = |name: String, duration: u32, this_rep, total_reps| {
            ans.push(FlatStatus {
                name,
                duration,
                this_rep,
                total_reps,
                absolute_start_time: start,
            });
            start += duration;
        };
        for rep in 0..self.reps {
            if rep > 0 {
                add(
                    format!("Rest between {}", self.name),
                    self.rest_between,
                    rep,
                    self.reps,
                );
            }
            match &self.content {
                Work::Seconds(x) => add(format!("{}", self.name), *x, rep + 1, self.reps),
                Work::Composite(v) => {
                    for x in v.iter().map(|x| x.describe()).flatten() {
                        add(x.name, x.duration, x.this_rep, x.total_reps);
                    }
                }
            }
        }
        ans
    }
}

pub fn joe_wicks() -> WorkoutItem {
    WorkoutItem {
        name: "Workout".into(),
        reps: 1,
        rest_between: 0,
        content: Work::Composite(vec![
            WorkoutItem {
                name: "Warm up".into(),
                reps: 1,
                rest_between: 0,
                content: Work::Seconds(5 * 60),
            },
            WorkoutItem {
                name: "Set".into(),
                reps: 2,
                rest_between: 120,
                content: Work::Composite(vec![WorkoutItem {
                    name: "Work".into(),
                    reps: 10,
                    rest_between: 30,
                    content: Work::Seconds(30),
                }]),
            },
            WorkoutItem {
                name: "Stretches".into(),
                reps: 1,
                rest_between: 0,
                content: Work::Seconds(5 * 60),
            },
        ]),
    }
}
#[cfg(test)]
mod test {
    use super::*;

    //#[test]
    pub fn test_joe_description() {
        assert_eq!(joe_wicks().describe(), Vec::<FlatStatus>::new());
    }
    #[test]
    pub fn joe_duration() {
        assert_eq!(joe_wicks().total_duration(), 31 * 60);
        let descr = joe_wicks().describe();
        let last = descr.last().unwrap();
        assert_eq!(last.duration + last.absolute_start_time, 31 * 60);
    }
}
