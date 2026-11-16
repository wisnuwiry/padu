//! One tool: what it is called, what it takes, and what it does.

use serde_json::Value;
use std::path::Path;

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn(bound: bool) -> Value,
    pub writes: bool,
    pub deletes: bool,
    pub call: fn(Args<'_>) -> Outcome,
}

#[derive(Clone, Copy)]
pub struct Arg {
    pub name: &'static str,
    pub about: &'static str,
}

pub type Outcome = Result<Answer, Trouble>;

#[derive(Debug, Clone)]
pub struct Answer {
    pub text: String,
    pub data: Option<Value>,
}

impl Answer {
    pub fn said(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            data: None,
        }
    }

    pub fn with(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    Invalid(String),
    Refused(String),
}

pub struct Args<'a> {
    arguments: &'a Value,
    at: Option<&'a Path>,
    session: Option<&'a str>,
}

impl<'a> Args<'a> {
    pub fn new(arguments: &'a Value, at: Option<&'a Path>) -> Self {
        Self {
            arguments,
            at,
            session: None,
        }
    }

    pub fn from_session(mut self, session: Option<&'a str>) -> Self {
        self.session = session;
        self
    }

    pub fn session(&self) -> Option<&'a str> {
        self.session
    }

    pub fn at(&self) -> Option<&'a Path> {
        self.at
    }

    pub fn text(&self, arg: Arg) -> Result<&'a str, Trouble> {
        match self.arguments.get(arg.name).and_then(Value::as_str) {
            Some(text) => Ok(text),
            None => Err(Trouble::Invalid(format!(
                "{} is required, as a string",
                arg.name
            ))),
        }
    }

    pub fn list(&self, arg: Arg) -> Result<Vec<&'a str>, Trouble> {
        match self.any_list(arg)? {
            many if many.is_empty() => Err(Self::not_a_list(arg)),
            many => Ok(many),
        }
    }

    pub fn any_list(&self, arg: Arg) -> Result<Vec<&'a str>, Trouble> {
        match self.arguments.get(arg.name) {
            Some(Value::String(one)) => Ok(vec![one.as_str()]),
            Some(Value::Array(many)) => many
                .iter()
                .map(|value| value.as_str().ok_or_else(|| Self::not_a_list(arg)))
                .collect(),
            _ => Err(Self::not_a_list(arg)),
        }
    }

    fn not_a_list(arg: Arg) -> Trouble {
        Trouble::Invalid(format!(
            "{} is required, as a string or a list of strings",
            arg.name
        ))
    }

    pub fn value(&self, arg: Arg) -> Option<&'a Value> {
        self.arguments.get(arg.name)
    }

    pub fn maybe(&self, arg: Arg) -> Option<&'a str> {
        self.arguments.get(arg.name).and_then(Value::as_str)
    }

    pub fn boolean(&self, arg: Arg, default: bool) -> Result<bool, Trouble> {
        match self.arguments.get(arg.name) {
            None => Ok(default),
            Some(Value::Bool(value)) => Ok(*value),
            Some(_) => Err(Trouble::Invalid(format!("{} must be a boolean", arg.name))),
        }
    }
}
