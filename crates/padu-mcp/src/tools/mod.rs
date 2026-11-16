//! The tool sets, one per surface a project holds.

pub mod browser;
pub mod workspace;

use crate::{
    rail,
    tool::{Arg, Args, Trouble},
};
use serde_json::{Value, json};
use std::path::Path;

pub(crate) const PROJECT: Arg = Arg {
    name: "project",
    about: "The project: the path of the directory the work is in. A session \
already in a project may leave this out to mean that one, and must name a \
project Padu has open to mean another.",
};

pub(crate) fn root<'a>(args: &'a Args<'a>) -> Result<std::path::PathBuf, Trouble> {
    if let Some(named) = args.maybe(PROJECT) {
        let p = Path::new(named);
        on_the_rail(p)?;
        return Ok(p.to_path_buf());
    }
    if let Some(at) = args.at() {
        return Ok(at.to_path_buf());
    }
    let open_projects = rail::open();
    if let Some(first) = open_projects.first() {
        return Ok(first.clone());
    }
    if let Ok(cwd) = std::env::current_dir() {
        return Ok(cwd);
    }
    Err(Trouble::Refused(
        "no project is open in Padu and no project path was provided".to_owned(),
    ))
}

pub(crate) fn on_the_rail(path: &Path) -> Result<&Path, Trouble> {
    if rail::is_open(path) {
        return Ok(path);
    }
    Err(Trouble::Refused(format!(
        "project {} is not open in Padu",
        path.display()
    )))
}

pub(crate) fn optional(schema: &mut Value, arg: Arg, kind: &str) {
    schema["properties"][arg.name] = json!({ "type": kind, "description": arg.about });
}

pub(crate) fn fields(bound: bool, args: &[Arg]) -> Value {
    let required: Vec<&str> = args
        .iter()
        .map(|arg| arg.name)
        .filter(|name| !(bound && *name == PROJECT.name))
        .collect();
    let properties = args
        .iter()
        .map(|arg| {
            (
                arg.name.to_owned(),
                json!({ "type": "string", "description": arg.about }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}
