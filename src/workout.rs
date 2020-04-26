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

#[derive(Debug,PartialEq,Eq)]
pub struct FlatStatus {
    pub name: String,
    pub this_rep: u32,
    pub total_reps: u32,
    pub duration: Option<u32>,
}

impl FlatStatus {
    pub fn is_rest(&self) -> bool {
        let lc = self.name.to_ascii_lowercase();
        lc.starts_with("rest") || lc.starts_with("recover") || lc.starts_with("end")
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
    pub fn describe<'a>(&'a self) -> Vec<FlatStatus> {
        let mut ans = Vec::new();
        let mut add = |name: &str, duration:Option<u32>, this_rep, total_reps| {
            ans.push(FlatStatus {
                name: name.to_string(),
                duration,
                this_rep,
                total_reps,
            });
        };
        for rep in 0..self.reps {
            if rep > 0 {
                add(&format!("Rest between {}", self.name), Some(self.rest_between), rep,self.reps);
            }
            match &self.content {
                Work::Seconds(x) =>
                    add( &format!( "{}", self.name), Some(*x),rep+1,self.reps),
                Work::Composite(v) => {
                    for x in v.iter().map(|x| x.describe()).flatten() {
                        add(&x.name, x.duration,x.this_rep,x.total_reps);
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
                content: Work::Seconds(5),
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

    #[test]
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
