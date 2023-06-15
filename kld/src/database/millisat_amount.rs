use std::{fmt, ops::Add};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct MillisatAmount(pub u64);

impl MillisatAmount {
    pub fn as_i64(&self) -> i64 {
        self.0 as i64
    }

    pub fn zero() -> Self {
        MillisatAmount(0)
    }
}

impl Add<MillisatAmount> for MillisatAmount {
    type Output = Self;

    fn add(self, rhs: MillisatAmount) -> Self::Output {
        MillisatAmount(self.0 + rhs.0)
    }
}

impl From<i64> for MillisatAmount {
    fn from(value: i64) -> Self {
        MillisatAmount(value as u64)
    }
}

impl fmt::Display for MillisatAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
