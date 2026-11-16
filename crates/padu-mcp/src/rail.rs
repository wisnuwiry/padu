//! The projects and browser the app has open, from a tool's side of the glass.

use crate::tool::Trouble;
use std::{
    path::{Path, PathBuf},
    sync::RwLock,
    time::Duration,
};

static HAND: RwLock<Option<Hand>> = RwLock::new(None);
static OPEN: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());
static SHOWN: RwLock<Vec<Shown>> = RwLock::new(Vec::new());
static BROWSER: RwLock<Option<Browser>> = RwLock::new(None);

const BROWSE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Open(PathBuf),
    Close(PathBuf),
    Send {
        session: String,
        message: String,
    },
    Rename {
        session: String,
        title: String,
    },
    Label {
        session: String,
        labels: Vec<String>,
    },
    Start {
        project: PathBuf,
        agent: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub project: PathBuf,
    pub kind: String,
    pub id: String,
    pub focused: bool,
}

type Hand = Box<dyn Fn(Change) + Send + Sync>;

pub fn install(hand: impl Fn(Change) + Send + Sync + 'static) {
    if let Ok(mut held) = HAND.write() {
        *held = Some(Box::new(hand));
    }
}

pub fn set_open(projects: Vec<PathBuf>) {
    if let Ok(mut held) = OPEN.write() {
        *held = projects;
    }
}

pub fn open() -> Vec<PathBuf> {
    OPEN.read().map(|held| held.clone()).unwrap_or_default()
}

pub fn set_shown(shown: Vec<Shown>) {
    let same = SHOWN.read().is_ok_and(|held| *held == shown);
    if !same && let Ok(mut held) = SHOWN.write() {
        *held = shown;
    }
}

pub fn shown() -> Vec<Shown> {
    SHOWN.read().map(|held| held.clone()).unwrap_or_default()
}

pub fn is_open(path: &Path) -> bool {
    let settled = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let want = settled(path);
    open()
        .iter()
        .any(|held| held == path || settled(held) == want)
}

pub fn ask(change: Change) -> Result<(), Trouble> {
    let held = HAND.read().map_err(|_| nobody())?;
    let Some(hand) = held.as_ref() else {
        return Err(nobody());
    };
    hand(change);
    Ok(())
}

fn nobody() -> Trouble {
    Trouble::Refused("no Padu window to open a project in".to_owned())
}

#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Tabs,
    Open(String),
    Read,
    Click(usize),
    Type {
        element: usize,
        text: String,
        enter: bool,
    },
    Scroll(f64),
    Console,
}

pub struct Browse {
    pub project: PathBuf,
    pub tab: Option<u64>,
    pub act: Act,
    pub reply: std::sync::mpsc::Sender<Result<String, String>>,
}

type Browser = Box<dyn Fn(Browse) + Send + Sync>;

pub fn install_browser(hand: impl Fn(Browse) + Send + Sync + 'static) {
    if let Ok(mut held) = BROWSER.write() {
        *held = Some(Box::new(hand));
    }
}

pub(crate) fn browse(project: PathBuf, tab: Option<u64>, act: Act) -> Result<String, Trouble> {
    let (reply, answer) = std::sync::mpsc::channel();
    {
        let held = BROWSER.read().map_err(|_| no_browser())?;
        let Some(hand) = held.as_ref() else {
            return Err(no_browser());
        };
        hand(Browse {
            project,
            tab,
            act,
            reply,
        });
    }
    match answer.recv_timeout(BROWSE_TIMEOUT) {
        Ok(answer) => answer.map_err(Trouble::Refused),
        Err(_) => Err(Trouble::Refused(
            "the browser did not answer in time".to_owned(),
        )),
    }
}

fn no_browser() -> Trouble {
    Trouble::Refused("no Padu window with a browser to work in".to_owned())
}
