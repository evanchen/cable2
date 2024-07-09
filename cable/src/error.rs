use std::io;

#[derive(Debug)]
pub enum Error {
    Feedback((u32, String)),
    Message(String),
    IoError(io::Error),
    FileNotExist,
}

impl From<String> for Error {
    fn from(str: String) -> Self {
        Error::Message(str)
    }
}

impl<'a> From<&'a str> for Error {
    fn from(str: &'a str) -> Self {
        Error::Message(str.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::IoError(error)
    }
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Feedback((id, msg)) => {
                write!(f, "Feedback {{ id={},msg={} }}", id, msg)
            }
            Error::Message(msg) => write!(f, "Message {{ {} }}", msg),
            Error::IoError(err) => {
                write!(f, "IoError {{ {} }}", err)
            }
            Error::FileNotExist => write!(f, "FileNotExist"),
        }
    }
}
