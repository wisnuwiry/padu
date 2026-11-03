//! The server: the tools a caller is offered and the frames it answers.

use crate::{
    proto::{self, Error, Request, Response},
    resources,
    tool::{Answer, Args, Tool, Trouble},
};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const NAME: &str = "padu";

pub struct Server {
    tools: Vec<&'static Tool>,
    switched: Vec<Switched>,
    deletes: Arc<AtomicBool>,
    writable: Arc<AtomicBool>,
}

struct Switched {
    tools: &'static [Tool],
    resource: &'static str,
    on: Arc<AtomicBool>,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            tools: Vec::new(),
            switched: Vec::new(),
            writable: Arc::new(AtomicBool::new(true)),
            deletes: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Server {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writable(mut self, switch: Arc<AtomicBool>) -> Self {
        self.writable = switch;
        self
    }

    pub fn deletes(mut self, switch: Arc<AtomicBool>) -> Self {
        self.deletes = switch;
        self
    }

    pub fn offered(&self) -> impl Iterator<Item = &'static Tool> {
        let writable = self.writable.load(Ordering::Relaxed);
        let deletes = self.deletes.load(Ordering::Relaxed);
        let switched = self
            .switched
            .iter()
            .filter(|set| set.on.load(Ordering::Relaxed))
            .flat_map(|set| set.tools.iter());
        self.tools
            .iter()
            .copied()
            .chain(switched)
            .filter(move |tool| writable || !tool.writes)
            .filter(move |tool| (writable && deletes) || !tool.deletes)
    }

    pub fn mount(mut self, tools: &'static [Tool]) -> Self {
        self.tools.extend(tools);
        self
    }

    pub fn mount_switched(
        mut self,
        tools: &'static [Tool],
        resource: &'static str,
        on: Arc<AtomicBool>,
    ) -> Self {
        self.switched.push(Switched {
            tools,
            resource,
            on,
        });
        self
    }

    pub fn hidden(&self) -> Vec<&'static str> {
        let mut hidden: Vec<&'static str> = self
            .switched
            .iter()
            .map(|set| set.resource)
            .filter(|resource| {
                !self
                    .switched
                    .iter()
                    .any(|set| set.resource == *resource && set.on.load(Ordering::Relaxed))
            })
            .collect();
        hidden.sort_unstable();
        hidden.dedup();
        hidden
    }

    pub fn handle(&self, req: &Request, at: Option<&Path>) -> Option<Response> {
        self.handle_from(req, at, None)
    }

    pub fn handle_from(
        &self,
        req: &Request,
        at: Option<&Path>,
        session: Option<&str>,
    ) -> Option<Response> {
        let id = req.id.clone()?;
        Some(match req.method.as_str() {
            "initialize" => Response::ok(id, self.initialize(at)),
            "notifications/initialized" => return None,
            "tools/list" => Response::ok(id, self.list(at.is_some())),
            "tools/call" => match self.invoke(req.params.as_ref(), at, session) {
                Ok(result) => Response::ok(id, result),
                Err(error) => Response::fail(id, error),
            },
            "resources/list" => match resources::list(req.params.as_ref(), &self.hidden()) {
                Ok(result) => Response::ok(id, result),
                Err(error) => Response::fail(id, error),
            },
            "resources/read" => match resources::read(req.params.as_ref(), &self.hidden()) {
                Ok(result) => Response::ok(id, result),
                Err(error) => Response::fail(id, error),
            },
            "prompts/list" => Response::ok(id, json!({ "prompts": [] })),
            method => Response::fail(id, Error::method_not_found(method)),
        })
    }

    pub fn call(&self, name: &str, arguments: Value, at: Option<&Path>) -> Result<Answer, Trouble> {
        self.call_from(name, arguments, at, None)
    }

    pub fn call_from(
        &self,
        name: &str,
        arguments: Value,
        at: Option<&Path>,
        session: Option<&str>,
    ) -> Result<Answer, Trouble> {
        let name = name
            .strip_prefix("padu__")
            .or_else(|| name.strip_prefix("padu_"))
            .unwrap_or(name);
        if let Some(tool) = self.offered().find(|tool| tool.name == name) {
            return (tool.call)(Args::new(&arguments, at).from_session(session));
        }
        if self
            .switched
            .iter()
            .any(|set| set.tools.iter().any(|tool| tool.name == name))
        {
            return Err(Trouble::Refused(format!(
                "{name} is switched off in Padu's settings"
            )));
        }
        if let Some(tool) = self.tools.iter().find(|tool| tool.name == name) {
            let why = match tool.deletes && self.writable.load(Ordering::Relaxed) {
                true => "deletes, and Padu is set not to let agents delete",
                false => "changes the project, and Padu is set to let agents read only",
            };
            return Err(Trouble::Refused(format!("{name} {why}")));
        }
        Err(Trouble::Invalid(format!("no tool {name}")))
    }

    fn initialize(&self, at: Option<&Path>) -> Value {
        let instructions = padu_prompts::tool_context(at.is_some(), &self.hidden());
        let capabilities = json!({ "tools": { "listChanged": false }, "resources": {} });
        json!({
            "protocolVersion": proto::VERSION,
            "capabilities": capabilities,
            "serverInfo": { "name": NAME, "version": env!("CARGO_PKG_VERSION") },
            "instructions": instructions,
        })
    }

    pub fn list(&self, bound: bool) -> Value {
        json!({
            "tools": self
                .offered()
                .map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": (tool.schema)(bound),
                }))
                .collect::<Vec<_>>(),
        })
    }

    fn invoke(
        &self,
        params: Option<&Value>,
        at: Option<&Path>,
        session: Option<&str>,
    ) -> Result<Value, Error> {
        let params = params.ok_or_else(|| Error::invalid_params("no params"))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::invalid_params("name is required, as a string"))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        match self.call_from(name, arguments, at, session) {
            Ok(answer) => Ok(result(&answer.text, false)),
            Err(Trouble::Refused(why)) => Ok(result(&why, true)),
            Err(Trouble::Invalid(why)) => Err(Error::invalid_params(why)),
        }
    }
}

fn result(text: &str, failed: bool) -> Value {
    let mut result = json!({ "content": [{ "type": "text", "text": text }] });
    if failed {
        result["isError"] = Value::Bool(true);
    }
    result
}
