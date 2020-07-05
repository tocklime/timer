use serde::Deserialize;

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct FlatStatus {
    pub name: String,
    pub this_rep: u32,
    pub total_reps: u32,
    pub duration: Option<u32>,
}

pub fn timer(duration: u64) -> String {
    format!("{}:{:02}", duration / 60, duration % 60)
}

impl FlatStatus {
    pub fn is_rest(&self) -> bool {
        let lc = self.name.to_ascii_lowercase();
        lc.starts_with("rest") || lc.starts_with("recover") || lc.starts_with("end")
    }
    pub fn rep_str(&self) -> String {
        if self.total_reps > 1 {
            format!("{}/{}", self.this_rep, self.total_reps)
        } else {
            "".into()
        }
    }
    pub fn dur_str(&self) -> String {
        if let Some(d) = self.duration {
            timer(d.into())
        } else {
            "".into()
        }
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
    }
}
