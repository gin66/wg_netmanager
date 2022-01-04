use std::error::Error;
use std::fmt;

pub type BoxResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct MyError {
    msg: &'static str,
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MyError is {}", self.msg)
    }
}

impl serde::ser::StdError for MyError {}

pub fn strerror<T>(msg: &'static str) -> BoxResult<T> {
    Err(Box::new(MyError { msg }))
}
